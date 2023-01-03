use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};

use bytes::*;
use std::collections::LinkedList;

const WS_PACKET_MAGIC: u8 = 0x88;
const WS_PACKET_VERSION: u8 = 0x01;
const WS_PACKET_HEADER_LENGTH: usize = 10;
// const WS_PACKET_CONTENT_MAX_LENGTH: u16 = u16::MAX - WS_PACKET_HEADER_LENGTH;

pub struct WSPacketHeader {
    pub magic: u8,

    // 版本
    pub version: u8,

    // 序号
    pub seq: u16,

    // cmd
    // CMD=0表示是response，大于0表示request
    pub cmd: u16,

    // 内容长度
    pub content_length: u32,
}

impl WSPacketHeader {
    pub fn new(seq: u16, cmd: u16, content_length: u32) -> Self {
        Self {
            magic: WS_PACKET_MAGIC,
            version: WS_PACKET_VERSION,
            seq,
            cmd,
            content_length,
        }
    }

    pub fn parse(mut buf: &[u8]) -> BuckyResult<Self> {
        let magic = buf.get_u8();
        if magic != WS_PACKET_MAGIC {
            let msg = format!("invalid ws packet header magic: v={}", magic);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        // TODO 版本判断
        let _version = buf.get_u8();

        let seq = buf.get_u16();
        let cmd = buf.get_u16();
        let content_length = buf.get_u32();

        Ok(Self::new(seq, cmd, content_length))
    }

    pub fn encode(&self, buf: &mut Vec<u8>) {
        buf.put_u8(self.magic);
        buf.put_u8(self.version);
        buf.put_u16(self.seq);
        buf.put_u16(self.cmd);
        buf.put_u32(self.content_length);
    }
}

pub struct WSPacket {
    pub header: WSPacketHeader,
    pub content: Vec<u8>,
}

impl WSPacket {
    pub fn new_from_bytes(seq: u16, cmd: u16, content: Vec<u8>) -> Self {
        let header = WSPacketHeader::new(seq, cmd, content.len() as u32);
        WSPacket { header, content }
    }

    pub fn encode(&self) -> Vec<u8> {
        assert_eq!(self.content.len(), self.header.content_length as usize);

        let total = WS_PACKET_HEADER_LENGTH + self.content.len();
        let mut buf = Vec::with_capacity(total);
        self.header.encode(&mut buf);
        buf.put_slice(&self.content);

        buf
    }

    pub fn decode(buf: Vec<u8>) -> BuckyResult<Self> {
        if buf.len() < WS_PACKET_HEADER_LENGTH {
            let msg = format!("invalid ws packet header len: buf len={}", buf.len());
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let header = WSPacketHeader::parse(&buf[0..WS_PACKET_HEADER_LENGTH])?;
        if header.content_length as usize != buf.len() - WS_PACKET_HEADER_LENGTH {
            let msg = format!(
                "invalid ws packet content len: except context len={}, got len={}",
                header.content_length,
                buf.len() - WS_PACKET_HEADER_LENGTH
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        Ok(Self {
            header,
            content: buf[WS_PACKET_HEADER_LENGTH..].to_vec(),
        })
    }
}

enum WSPacketParserState {
    Header,
    Body,
}

pub struct WSPacketParser {
    buf: BytesMut,
    remain: usize,
    state: WSPacketParserState,
    header: Option<WSPacketHeader>,

    packets: LinkedList<WSPacket>,
}

impl WSPacketParser {
    pub fn new() -> Self {
        Self {
            buf: BytesMut::with_capacity(2000),
            remain: WS_PACKET_HEADER_LENGTH,
            state: WSPacketParserState::Header,
            header: None,
            packets: LinkedList::new(),
        }
    }

    pub fn next_packet(&mut self) -> Option<WSPacket> {
        self.packets.pop_front()
    }

    pub fn push(&mut self, mut buf: &[u8]) -> BuckyResult<()> {
        loop {
            if buf.len() < self.remain {
                self.remain -= buf.len();
                self.buf.put(buf);
                break;
            } else {
                self.buf.put(&buf[..self.remain]);
                buf = &buf[self.remain..];
            }

            match self.state {
                WSPacketParserState::Header => {
                    self.state = WSPacketParserState::Body;

                    let header =
                        WSPacketHeader::parse(&mut self.buf[0..WS_PACKET_HEADER_LENGTH])?;
                    self.remain = header.content_length as usize;

                    assert!(self.header.is_none());
                    self.header = Some(header);
                }
                WSPacketParserState::Body => {
                    self.state = WSPacketParserState::Header;
                    self.remain = WS_PACKET_HEADER_LENGTH;

                    assert!(self.header.is_some());
                    let header = self.header.take().unwrap();
                    let content = self.buf[..header.content_length as usize].to_owned();

                    trace!("ws recv packet: seq={}, len={}", header.seq, content.len());

                    let packet = WSPacket { header, content };
                    self.packets.push_back(packet);
                }
            }

            unsafe {
                self.buf.set_len(0);
            }
        }

        Ok(())
    }
}
