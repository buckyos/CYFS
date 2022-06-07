use log::*;
use std::{
    collections::LinkedList, 
};
use crate::{
    protocol::*
};
use super::stream::PackageStream;

struct Block {
    data: Vec<u8>, 
    start: u64, 
    fin: bool
}

impl Block {
    fn end(&self) -> u64 {
        self.start + self.data.len() as u64
    }
}


pub struct RecvQueue {
    capability: u64, 
    start: u64, 
    stream_writer: ringbuf::Producer<u8>, 
    stream_reader: ringbuf::Consumer<u8>, 
    blocks: LinkedList<Block>
}

impl RecvQueue {
    pub fn new(capability: usize) -> Self {
        let stream_buffer = ringbuf::RingBuffer::new(capability);
        let (stream_writer, stream_reader) = stream_buffer.split();
        Self {
            capability: capability as u64, 
            start: 0, 
            stream_writer, 
            stream_reader, 
            blocks: LinkedList::new()
        }
    }

    pub fn push(&mut self, stream: &PackageStream, pkg: &SessionData) -> (usize, bool) {
        if !pkg.is_flags_contain(SESSIONDATA_FLAG_FIN) {
            if pkg.payload.as_ref().len() == 0 {
                trace!("{} recv queue ignore session data for no payload", stream);
                return (0, false);
            }
            if pkg.stream_pos_end() <= self.stream_end() {
                trace!("{} recv queue ignore session data for lower than low wnd {}", stream, self.stream_end());
                return (0, false);
            }
            if pkg.stream_pos_end() > (self.start + self.capability) {
                trace!("{} recv queue ignore session data for upper than high wnd {}", stream, (self.start + self.capability));
                return (0, false);
            }
        } 
        if pkg.stream_pos == self.stream_end() {
            self.stream_writer.push_slice(&pkg.payload.as_ref()[..]);
            
            let mut fin = false;
            if pkg.is_flags_contain(SESSIONDATA_FLAG_FIN) {
                assert_eq!(self.blocks.len(), 0);
                fin = true;
            } else {
                let mut stream_start = self.stream_end();
                let mut stream_index = None;
                for (index, block) in (&self.blocks).iter().enumerate() {
                    if block.start == stream_start {
                        stream_start = block.end();
                        stream_index = Some(index);
                        fin = block.fin;
                    } else {
                        break;
                    }
                }
                if let Some(stream_index) = stream_index {
                    let blocks = self.blocks.split_off(stream_index + 1);
                    for block in &self.blocks {
                        self.stream_writer.push_slice(&block.data[..]);
                    }
                    self.blocks = blocks;
                    trace!("{} recv queue extend recved block to stream buffer to {}", stream, self.stream_end());
                }
            }

            trace!("{} recv queue append session data to stream buffer to {}", stream, self.stream_end());
            ((self.stream_end() - pkg.stream_pos) as usize, fin)
        } else {
            trace!("{} recv queue add session data to recved block", stream);
            let block = self.alloc_block(pkg);
            if self.blocks.len() == 0 {
                self.blocks.push_back(block);
            } else if !self.blocks.back().unwrap().fin {
                if pkg.stream_pos >= self.blocks.back().unwrap().end() {
                    self.blocks.push_back(block);
                } else {
                    let mut insert_before = None;
                    for (index, block) in (&self.blocks).iter().enumerate() {
                        if pkg.stream_pos == block.start {
                            break;
                        }
                        else if pkg.stream_pos_end() <= block.start {
                            insert_before = Some(index);
                            break;
                        } 
                    }
                    if let Some(insert_before) = insert_before {
                        let mut back_parts = self.blocks.split_off(insert_before);
                        self.blocks.push_back(block);
                        self.blocks.append(&mut back_parts);
                    } 
                }
            }
            (0, false)
        }
    }

    pub fn stream_end(&self) -> u64 {
        self.start + self.stream_writer.len() as u64
    }

    fn alloc_block(&mut self, pkg: &SessionData) -> Block {
        Block {
            data: Vec::from(pkg.payload.as_ref()), 
            start: pkg.stream_pos, 
            fin: pkg.is_flags_contain(SESSIONDATA_FLAG_FIN)
        }
    }

    pub fn stream_len(&self) -> usize {
        self.stream_reader.len()
    }

    pub fn read_stream(&mut self, buf: &mut [u8]) -> usize {
        let read = self.stream_reader.pop_slice(buf);
        self.start += read as u64;
        read
    }
}