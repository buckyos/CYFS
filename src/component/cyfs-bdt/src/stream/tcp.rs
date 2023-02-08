use log::*;
use std::{
    time::Duration, 
    sync::{atomic::{AtomicPtr, Ordering}, Mutex}, 
    task::{Context, Poll, Waker}, 
    collections::LinkedList
};
use async_std::{
    sync::{Arc},
    channel::{bounded, Sender, Receiver},
    task, 
    future, 
    io::prelude::{WriteExt, ReadExt},
};
use ringbuf;
use async_trait::{async_trait};
use cyfs_base::*;
// use cyfs_debug::Mutex;
use crate::{
    types::*, 
    protocol::{*, v0::*}, 
};
use super::{
    container::StreamContainer, 
    stream_provider::{Shutdown, StreamProvider}};

#[derive(Clone)]
pub struct Config {
    pub max_record: u16, 
    pub min_record: u16,
}

struct SendQueue {
    state: SendQueueState, 
    nagle_id_generator: IncreaseIdGenerator, 
    write_waiter: Option<(Waker, usize)>, 
    flush_waiters: LinkedList<Waker>, 
    close_waiters: LinkedList<Waker>, 
    buffer: Vec<u8>, 
    record_count: usize,  
    sending_state: SendingState
}

enum SendQueueState {
    Open, 
    Closing
}

enum SendingState {
    Idle, 
    Recording(IncreaseId, u16/*record size*/), 
    Sending, 
}

struct ToSend {
    data_ptr: *const u8, 
    data_len: usize, 
    buffer_ptr: *mut u8, 
    buffer_len: usize, 
    exists_len: usize
}

struct WriteResult {
    poll: Poll<std::io::Result<usize>>, 
    nagle: Option<IncreaseId>, 
    buffer: Option<ToSend>
}

fn box_header_len() -> usize {
    u16::raw_bytes().unwrap()
}

impl SendQueue {
    fn new(config: &super::container::Config) -> Self {
        let record_count = config.send_buffer / config.tcp.max_record as usize + 1;
        let send_buffer = record_count * (box_header_len() + AesKey::padded_len(config.tcp.max_record as usize));
        Self {
            state: SendQueueState::Open,
            nagle_id_generator: IncreaseIdGenerator::new(), 
            write_waiter: None, 
            flush_waiters: LinkedList::new(), 
            close_waiters: LinkedList::new(), 
            buffer: vec![0u8; send_buffer], 
            record_count, 
            sending_state: SendingState::Idle
        }
    }

    fn buffer_len(&self, config: &super::container::Config) -> usize {
        config.tcp.max_record as usize * self.record_count
    }

    fn append_record(&mut self, from: u16, data: &[u8]) {
        self.buffer[box_header_len() + from as usize..box_header_len() + from as usize + data.len()].copy_from_slice(data);
    }

    fn flush(&mut self, waker: &Waker) -> Poll<std::io::Result<()>> {
        match self.state {
            SendQueueState::Open => {
                match &mut self.sending_state {
                    SendingState::Idle => {
                        Poll::Ready(Ok(()))
                    }, 
                    _ => {
                        self.flush_waiters.push_back(waker.clone());
                        Poll::Pending
                    }
                }
            }, 
            SendQueueState::Closing => {
                Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "send queue closing")))
            }
        }
        
    }

    fn write(&mut self, stream: &TcpStream, buf: &[u8], waker: &Waker) -> WriteResult {
        match self.state {
            SendQueueState::Open => {
                match self.sending_state {
                    SendingState::Idle => {
                        if buf.len() >= stream.config().tcp.min_record as usize {
                            debug!("{} write {} bytes larger than min record, send state idle=>sending", stream, buf.len());
                            self.sending_state = SendingState::Sending;
                            let buffer_len = self.buffer_len(stream.config());
                            let data_len = {
                                if buffer_len > buf.len() {
                                    buf.len()
                                } else {
                                    buffer_len
                                }
                            };
                            WriteResult {
                                poll: Poll::Ready(Ok(data_len)), 
                                nagle: None, 
                                buffer: Some(ToSend {
                                    data_ptr: buf.as_ptr(), 
                                    data_len, 
                                    buffer_ptr: self.buffer.as_mut_ptr(), 
                                    buffer_len: self.buffer.len(), 
                                    exists_len: 0
                                })
                            }
                        } else {
                            self.append_record(0, buf);
                            let nagle_id = self.nagle_id_generator.generate();
                            debug!("{} write {} bytes less than min record, send state idle=>recording(id={},len={})", stream, buf.len(), nagle_id, buf.len());
                            self.sending_state = SendingState::Recording(nagle_id, buf.len() as u16);
                            WriteResult {
                                poll: Poll::Ready(Ok(buf.len())), 
                                nagle: Some(nagle_id), 
                                buffer: None
                            }
                        }   
                    },
                    SendingState::Recording(nagle_id, exists_len) => {
                        let total = exists_len as usize + buf.len();
                        if total < stream.config().tcp.min_record as usize {
                            self.append_record(exists_len, buf);
                            if let SendingState::Recording(_, ref mut exists_len) = self.sending_state {
                                let rep_exists_len = *exists_len;
                                *exists_len = total as u16;
                                debug!("{} write {} bytes in recording(id={},len={})=>recording(id={},len={})", stream, buf.len(), nagle_id, rep_exists_len, nagle_id, *exists_len);
                            } else {
                                unreachable!()
                            }
                            WriteResult {
                                poll: Poll::Ready(Ok(buf.len())), 
                                nagle: None, 
                                buffer: None
                            }
                        } else {
                            let append_len = {
                                let append_len = (stream.config().tcp.max_record - exists_len) as usize;
                                if append_len > buf.len() {
                                    buf.len()
                                } else {
                                    append_len
                                }
                            };
                            debug!("{} write {} bytes from recording(id={},len={}) to sending", stream, buf.len(), nagle_id, exists_len);
                            self.append_record(exists_len, &buf[..append_len]);
                            self.sending_state = SendingState::Sending;
                            let data_len = {
                                if append_len == buf.len() {
                                    0
                                } else {
                                    let data_len = buf.len() - append_len;
                                    let buffer_len = (self.record_count - 1) * stream.config().tcp.max_record as usize;
                                    if data_len > buffer_len {
                                        buffer_len
                                    } else {
                                        data_len
                                    }
                                }
                            };
                            WriteResult {
                                poll: Poll::Ready(Ok(append_len + data_len)), 
                                nagle: None, 
                                buffer: Some(ToSend {
                                    data_ptr: unsafe {buf.as_ptr().offset(append_len as isize)}, 
                                    data_len, 
                                    buffer_ptr: self.buffer.as_mut_ptr(),
                                    buffer_len: self.buffer.len(), 
                                    exists_len: exists_len as usize + append_len})
                            }
                        }
                    }, 
                    SendingState::Sending => {
                        debug!("{} write {} bytes pending for sending", stream, buf.len());
                        let poll = if self.write_waiter.is_some() {
                            let msg = format!("{} pending write for former pending write", stream);
                            error!("{}", msg.as_str());
                            Poll::Pending
                        } else {
                            self.write_waiter = Some((waker.clone(), buf.len()));
                            Poll::Pending
                        };
                        
                        WriteResult {
                            poll, 
                            nagle: None, 
                            buffer: None
                        }
                    }
                }
            },
            SendQueueState::Closing => {
                WriteResult {
                    poll: Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "send queue closing"))), 
                    nagle: None, 
                    buffer: None
                }
            }
        }
    }

    fn on_nagle(&mut self, stream: &TcpStream, nagle_id: IncreaseId) -> Option<ToSend> {
        match &self.sending_state {
            SendingState::Recording(stub_nagle_id, exists_len) => {
                if nagle_id == *stub_nagle_id {
                    trace!("{} send recording(id={},len={})=>sending for nagle", stream, stub_nagle_id, exists_len);
                    let exists_len = *exists_len as usize;
                    self.sending_state = SendingState::Sending;

                    Some(ToSend {
                        data_ptr: std::ptr::null(), 
                        data_len: 0, 
                        buffer_ptr: self.buffer.as_mut_ptr(),
                        buffer_len: self.buffer.len(), 
                        exists_len 
                    })
                } else {
                    None
                }
            },
            _ => None
        }
    }

    fn on_sent(&mut self, stream: &TcpStream) -> Option<LinkedList<Waker>> {
        match self.state {
            SendQueueState::Open => {
                match &mut self.sending_state {
                    SendingState::Sending => {
                        trace!("{} sending=>idle", stream);
                        self.sending_state = SendingState::Idle;
                        let mut to_wake = LinkedList::new();
                        if let Some(stub) = self.write_waiter.as_ref() {
                            to_wake.push_back(stub.0.clone());
                        }
                        self.write_waiter = None;
                        to_wake.append(&mut self.flush_waiters);
                        Some(to_wake)
                    }, 
                    _ => {
                        unreachable!()
                    }
                }
            }, 
            SendQueueState::Closing => None
        }
    }

    fn close(&mut self, waker: Option<&Waker>) -> Option<(Option<LinkedList<Waker>>, Option<ToSend>)> {
        match self.state {
            SendQueueState::Open => {
                let mut to_wake = LinkedList::new();
                if let Some(stub) = self.write_waiter.as_ref() {
                    to_wake.push_back(stub.0.clone());
                } 
                self.write_waiter = None;
                to_wake.append(&mut self.flush_waiters);
                self.state = SendQueueState::Closing;
                match &self.sending_state {
                    SendingState::Idle => None, 
                    SendingState::Recording(_, exists_len) => {
                        if waker.is_some() {
                            self.close_waiters.push_back(waker.unwrap().clone());
                        }
                        let exists_len = *exists_len as usize;
                            self.sending_state = SendingState::Sending;
        
                            Some((Some(to_wake), Some(ToSend {
                                data_ptr: std::ptr::null(), 
                                data_len: 0, 
                                buffer_ptr: self.buffer.as_mut_ptr(),
                                buffer_len: self.buffer.len(), 
                                exists_len 
                            })))
                    },
                    SendingState::Sending => {
                        if waker.is_some() {
                            self.close_waiters.push_back(waker.unwrap().clone());
                        }
                        Some((Some(to_wake), None))
                    }
                }
            }, 
            SendQueueState::Closing => {
                if waker.is_some() {
                    self.close_waiters.push_back(waker.unwrap().clone());
                }
                Some((None, None))
            }
        }
    } 
}

struct ReadStub {
    waker: Waker, 
    len: usize, 
    time: u64
}

struct RecvQueue {
    remote_closed: bool,
    timeout: bool,      
    data_consumer: ringbuf::Consumer<u8>, 
    readable_waiter: Option<Waker>, 
    read_waiter: Option<ReadStub>, 
    record_waiter: Sender<usize>
}

impl RecvQueue {
    fn new(config: &super::container::Config) -> (Self, Receiver<usize>, ringbuf::Producer<u8>) {
        let ring_buf = ringbuf::RingBuffer::<u8>::new(config.recv_buffer);
        let (data_producer, data_consumer) = ring_buf.split();
        let (record_waiter, record_waker) = bounded(100);
        (Self {
            remote_closed: false,
            timeout: false,  
            data_consumer, 
            readable_waiter: None, 
            read_waiter: None, 
            record_waiter 
        }, record_waker, data_producer)
    }

    async fn recv_record<'a>(socket: &mut async_std::net::TcpStream, key: &AesKey,  
        header_buffer: &'a mut [u8], record_buffer: &'a mut [u8]) -> Result<usize, BuckyError> {
        let r = socket.read_exact(header_buffer).await;
        if r.is_err() {
            let err = r.unwrap_err();
            if err.kind() == std::io::ErrorKind::UnexpectedEof {
                return Ok(0);
            } else {
                return Err(BuckyError::from(err));
            }
        } 
        let (len, _) = u16::raw_decode(header_buffer)?;
        let len = len as usize;
        let _ = socket.read_exact(&mut record_buffer[..len]).await?;
        let len = key.inplace_decrypt(record_buffer, len)?;
        Ok(len)
    }

    async fn start(
        stream: TcpStream, 
        waiter: Receiver<usize>, 
        data_producer: ringbuf::Producer<u8>) {
        let mut socket = stream.0.socket.clone();
        let config = &stream.0.config;
        let key = &stream.0.key;
        let mut data_producer = data_producer;

        let mut header_buffer = vec![0u8; box_header_len()];
        let mut record_buffer = vec![0u8; AesKey::padded_len(config.tcp.max_record as usize)]; // TODO, padded_len 调用变化
        loop {
            if data_producer.len() + config.tcp.max_record as usize > data_producer.capacity() {
                trace!("{} recv buffer full waiting", stream);
                match waiter.recv().await {
                    Ok(_) => {
                        trace!("{} recv record loop continue", stream);
                        continue;
                    },
                    Err(_) => {
                        debug!("{} recv record loop break for recv queue closed", stream);
                        break;
                    }
                };
                // let _ = future::timeout(Duration::from_millis(100), future::pending::<()>()).await;
            } else {
                trace!("{} pre recv record", stream);
                match Self::recv_record(&mut socket, key, &mut header_buffer[..], &mut record_buffer[..]).await {
                    Ok(len) => {
                        if len == 0 {
                            debug!("{} recv record bytes {}", stream, len);
                        } else {
                            trace!("{} recv record bytes {}", stream, len);
                        }
                        
                        data_producer.push_slice(&record_buffer[..len]);

                        let (readable_waker, read_waker) = {
                            let read_provider = &mut *cyfs_debug::lock!(stream.0.read_provider).unwrap();
                            match read_provider {
                                PollReadProvider::Open(queue) => {
                                    let mut readable_waker = None;
                                    std::mem::swap(&mut queue.readable_waiter, &mut readable_waker);
                                    if len == 0 {
                                        queue.remote_closed = true;
                                    }
                                    (readable_waker, Some(queue.wake(&stream, config, data_producer.len())))
                                }, 
                                PollReadProvider::Closed(_) => (None, None)
                            }
                        };

                        if let Some(to_wake) = readable_waker {
                            to_wake.wake();
                        }

                        if let Some(to_wake) = read_waker {
                            if let Some(to_wake) = to_wake {
                                to_wake.wake();
                            }
                            if len == 0 {
                                debug!("{} recv record loop break for remote closed", stream);
                                break;
                            }
                        } else {
                            debug!("{} recv record loop break recv queue closed", stream);
                            break;
                        }
                    }, 
                    Err(err) => {
                        debug!("{} recv record loop break for err {}", stream, err);
                        let to_wake = {
                            let read_provider = &mut *cyfs_debug::lock!(stream.0.read_provider).unwrap();
                            let mut to_wake = LinkedList::new();
                            match read_provider {
                                PollReadProvider::Open(queue) => {
                                    if queue.readable_waiter.is_some() {
                                        to_wake.push_back(queue.readable_waiter.as_ref().unwrap().clone());
                                    }
                                    if queue.read_waiter.is_some() {
                                        to_wake.push_back(queue.read_waiter.as_ref().unwrap().waker.clone());
                                    }
                                    *read_provider = PollReadProvider::Closed(Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe)));
                                }
                                _ => {}
                            };
                            to_wake
                        };
                        for waker in to_wake {
                            waker.wake();
                        }
                        break;
                    }
                }
            } 
        }
    } 
    
    fn wake(&mut self, stream: &TcpStream, config: &super::container::Config, total: usize) -> Option<Waker> {
        let now = bucky_time_now();
        let mut to_wake = None;
        if self.remote_closed {
            to_wake = self.read_waiter.as_ref().map(|stub| stub.waker.clone());
            self.read_waiter = None;
        } else if let Some(stub) = self.read_waiter.as_ref() {
            if total >= stub.len {
                debug!("{} wake read at {} {} bytes for enough buffer", stream, stub.time, stub.len);
                to_wake = Some(stub.waker.clone());
                self.read_waiter = None;
            } else if (now >= stub.time) && (Duration::from_micros(now - stub.time) > config.recv_timeout) {
                debug!("{} wake read at {} {} bytes for timeout", stream, stub.time, stub.len);
                self.timeout = true;
                to_wake = Some(stub.waker.clone());
                self.read_waiter = None;
            } else if total > config.recv_drain() {
                debug!("{} wake read at {} {} bytes for drain", stream, stub.time, stub.len);
                to_wake = Some(stub.waker.clone());
                self.read_waiter = None;
            }
        }
        
        to_wake
    }

    fn check_timeout(&mut self, stream: &TcpStream, stub_time: u64) -> Option<Waker> {
        trace!("{} checking timeout of read at {}", stream, stub_time);
        if self.timeout {
            trace!("{} checking timeout of read at {} has another timeout", stream, stub_time);
            return None;
        }
        if self.remote_closed {
            trace!("{} checking timeout of read at {} remote closed", stream, stub_time);
            return None;
        }
        if self.read_waiter.is_none() {
            trace!("{} checking timeout of read at {} waiter has removed", stream, stub_time);
            return None;
        }
        let stub = self.read_waiter.as_ref().unwrap();
        if self.data_consumer.len() == 0 {
            trace!("{} checking timeout of read at {} no data", stream, stub_time);
            return None;
        }
        if stub.time != stub_time {
            trace!("{} checking timeout of read at {} waiter has changed", stream, stub_time);
            return None;
        }
        debug!("{} wake read at {} {} bytes for timeout", stream, stub_time, stub.len);
        self.timeout = true;
        let waiter = Some(stub.waker.clone());
        self.read_waiter = None;
        waiter
    }

    fn read(&mut self, stream: &TcpStream, waker: &Waker, buf: &mut [u8]) -> Poll<std::io::Result<usize>> {
        if self.read_waiter.is_some() {
            let msg = format!("{} pending read for former pending read", stream);
            error!("{}", msg.as_str());
            return Poll::Pending;
        }
        let former_len = self.data_consumer.len();
        let config = &stream.0.config;
        if former_len >= buf.len() {
            debug!("{} read {} bytes ready for buffer enough", stream, buf.len());
            self.timeout = false;
        } else if former_len > config.recv_drain() {
            debug!("{} read {} bytes ready for buffer greater than drain", stream, buf.len());
            self.timeout = false;
        } else if self.remote_closed {
            debug!("{} read {} bytes ready for remote closed", stream, buf.len());
            self.timeout = false;
        } else if self.timeout {
            debug!("{} read {} bytes ready for timeout", stream, buf.len());
            self.timeout = false;
        } else {
            let stub_time = bucky_time_now();
            self.read_waiter = Some(ReadStub {
                waker: waker.clone(), 
                len: buf.len(), 
                time: stub_time
            });
            debug!("{} pending read at {} for no enough buffer {} ", stream, stub_time, former_len);
            let stream = stream.clone();
            task::spawn(async move {
                let to_wake = {
                    let _ = future::timeout(stream.0.config.recv_timeout, future::pending::<()>()).await;
                    
                    let read_provider = &mut *cyfs_debug::lock!(stream.0.read_provider).unwrap();
                    let to_wake = match read_provider {
                        PollReadProvider::Open(queue) => {
                            queue.check_timeout(&stream, stub_time)
                        }, 
                        PollReadProvider::Closed(_) => {
                            None
                        }
                    };
                   
                    to_wake
                };
                if let Some(to_wake) = to_wake {
                    to_wake.wake();
                }
            });
            return Poll::Pending;
        }
       
        let recv_len = self.data_consumer.pop_slice(buf);
        
        if former_len + config.tcp.max_record as usize > self.data_consumer.capacity() 
            && self.data_consumer.len() + config.tcp.max_record as usize <= self.data_consumer.capacity() {
            trace!("{} self read drain, wake recv record loop", stream);
            let r = self.record_waiter.try_send(0);
            assert_eq!(r.is_ok(), true);
        }

        debug!("{} poll read return {} bytes", stream, recv_len);
        Poll::Ready(Ok(recv_len))
    }

    fn readable(&mut self, waker: &Waker) -> Poll<std::io::Result<usize>> {
        if self.data_consumer.len() > 0 {
            Poll::Ready(Ok(self.data_consumer.len()))
        } else {
            assert!(self.readable_waiter.is_none());
            self.readable_waiter = Some(waker.clone());
            Poll::Pending
        }
    }
}

enum PollReadProvider {
    Open(RecvQueue), 
    Closed(std::io::Result<usize>)
}

struct TcpStreamImpl {
    owner: StreamContainer, 
    config: super::container::Config,
    local: Endpoint, 
    remote: Endpoint, 
    key: AesKey, 
    socket: async_std::net::TcpStream, 
    send_queue: Mutex<Option<SendQueue>>, 
    read_provider: Mutex<PollReadProvider>, 
}

#[derive(Clone)]
pub struct TcpStream(Arc<TcpStreamImpl>);

impl std::fmt::Display for TcpStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TcpStream:{{local:{}, remote:{}}}", self.0.local, self.0.remote)
    }
}

impl TcpStream {
    pub fn new(
        owner: StreamContainer, 
        socket: async_std::net::TcpStream, 
        key: AesKey) -> BuckyResult<Self> {
        let local = Endpoint::from((Protocol::Tcp, socket.local_addr()?));
        let remote = Endpoint::from((Protocol::Tcp, socket.peer_addr()?));
        let stack = owner.stack();
        let config = stack.config().stream.stream.clone();
        let send_queue = SendQueue::new(&config);
        let (recv_queue, record_waker, data_producer) = RecvQueue::new(&config);
        
        let stream = Self(Arc::new(TcpStreamImpl {   
            owner, 
            config, 
            local, 
            remote, 
            key, 
            socket, 
            send_queue: Mutex::new(Some(send_queue)), 
            read_provider: Mutex::new(PollReadProvider::Open(recv_queue))
        }));
        
        {
            let stream = stream.clone();
            task::spawn(async move {
                RecvQueue::start(
                    stream, 
                    record_waker, 
                    data_producer).await;
            });
        }
        
        Ok(stream)
    }

    async fn on_nagle(&self, nagle_id: IncreaseId) {
        let record = { 
            if let Some(send_queue) = &mut *cyfs_debug::lock!(self.0.send_queue).unwrap() {
                send_queue.on_nagle(self, nagle_id)
            } else {
                None
            }
        }.map(|to_send| self.encode_record(&to_send));
          
        if let Some((record_ptr, record_len)) = record {
            self.send_inner(record_ptr, record_len).await;
        }
    }

    fn encode_record(&self, to_send: &ToSend) -> (AtomicPtr<u8>, usize) {
        trace!("{} encode_record {{data_len:{},exists_len:{}}}", self, to_send.data_len, to_send.exists_len);
        let _config = &self.0.config;
        let mut buffer = unsafe {
            std::slice::from_raw_parts_mut(to_send.buffer_ptr, to_send.buffer_len)
        };
        if to_send.exists_len != 0 {
            let record_len = self.0.key.inplace_encrypt(&mut buffer[box_header_len()..], to_send.exists_len).unwrap();
            let _ = (record_len as u16).raw_encode(buffer, &None).unwrap();
            buffer = &mut buffer[box_header_len() + record_len..];
        }

        let mut data = unsafe {std::slice::from_raw_parts(to_send.data_ptr, to_send.data_len)};
        while data.len() > 0 {
            let data_len = {
                if data.len() > self.0.config.tcp.max_record as usize {
                    self.0.config.tcp.max_record as usize
                } else {
                    data.len()
                }
            };
            let record_len = self.0.key.encrypt(&data, &mut buffer[box_header_len()..], data_len).unwrap();
            let _ = (record_len as u16).raw_encode(buffer, &None).unwrap();
            buffer = &mut buffer[box_header_len() + record_len..];
            data = &data[data_len..];
        }
        
        (AtomicPtr::new(to_send.buffer_ptr), to_send.buffer_len - buffer.len())
    }

    async fn send_inner(&self, record_ptr: AtomicPtr<u8>, record_len: usize) {
        let record_buffer = unsafe {
            std::slice::from_raw_parts(record_ptr.load(Ordering::SeqCst), record_len)
        };
        let mut socket = self.0.socket.clone();
        trace!("{} pre send {} bytes", self, record_len);
        let (waiters, result) = match socket.write_all(record_buffer).await {
            Ok(_) => {
                trace!("{} sent {} bytes", self, record_len);

                let send_queue = &mut *cyfs_debug::lock!(self.0.send_queue).unwrap();
                let waiter = send_queue.as_mut().unwrap().on_sent(self);
                if let Some(waiter) = waiter {
                    (Some(waiter), Ok(()))
                } else {
                    let _ = self.0.socket.shutdown(Shutdown::Write);
                    *send_queue = None;
                    (None, Ok(()))
                }
                
            }, 
            Err(err) => {
                error!("{} sent failed for {}", self, err);
                let send_queue = &mut *cyfs_debug::lock!(self.0.send_queue).unwrap();
                let mut waiters = LinkedList::new(); 
                {
                    let send_queue = send_queue.as_mut().unwrap();
                    waiters.append(&mut send_queue.close_waiters);
                    waiters.append(&mut send_queue.flush_waiters);
                    if let Some(stub) = send_queue.write_waiter.as_ref() {
                        waiters.push_back(stub.0.clone());
                    }
                    send_queue.write_waiter = None;
                }
                *send_queue = None;
                (Some(waiters), Err(err))
            }
        };
        

        if let Some(waiters) = waiters {
            for waiter in waiters {
                waiter.wake();
            }
        }

        if result.is_err() {
            let _ = self.close_write(None);
            self.close_read();
            self.0.owner.break_with_error(result.unwrap_err().into())
        }
    }

    fn close_read(&self) {
        let to_wake = {
            let read_provider = &mut *cyfs_debug::lock!(self.0.read_provider).unwrap();
            let mut to_wake = LinkedList::new();
            match read_provider {
                PollReadProvider::Open(queue) => {
                    if queue.readable_waiter.is_some() {
                        to_wake.push_back(queue.readable_waiter.as_ref().unwrap().clone());
                    }
                    if queue.read_waiter.is_some() {
                        to_wake.push_back(queue.read_waiter.as_ref().unwrap().waker.clone());
                    }
                    *read_provider = PollReadProvider::Closed(Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "read closed")));
                }, 
                _ => {}
            };
            to_wake
        };
        for waker in to_wake {
            waker.wake();
        }
        let _ = self.0.socket.shutdown(Shutdown::Read);
    } 

    fn close_write(&self, waker: Option<&Waker>) -> Poll<std::io::Result<()>> {
        let r = {
            if let Some(send_queue) = &mut *cyfs_debug::lock!(self.0.send_queue).unwrap() {
                send_queue.close(waker)
            } else {
                None
            }
        };

        if let Some((to_wake, to_send)) = r {
            to_send.as_ref().map(|to_send| {
                let stream = self.clone();
                let (record_ptr, record_len) = stream.encode_record(to_send);
                task::spawn(async move {
                    stream.send_inner(record_ptr, record_len).await;
                });
            });
            to_wake.map(|to_wake| {
                for waiter in to_wake {
                    waiter.wake();
                }
            });
            Poll::Pending
        } else {
            let _ = self.0.socket.shutdown(Shutdown::Write);
            Poll::Ready(Ok(()))
        }
    }

    fn config(&self) -> &super::container::Config {
        &self.0.config
    }
}


#[async_trait]
impl StreamProvider for TcpStream {
    fn remote_id(&self) -> IncreaseId {
        IncreaseId::default()
    }

    fn local_ep(&self) -> &Endpoint {
        &self.0.local
    }

    fn remote_ep(&self) -> &Endpoint {
        &self.0.remote
    }

    fn start(&self, _owner: &StreamContainer) {

    }

    fn shutdown(&self, which: Shutdown, owner: &StreamContainer) -> Result<(), std::io::Error> {
        match which {
            Shutdown::Write => {
                let _ = self.close_write(None);
            }, 
            Shutdown::Read => {
                self.close_read();
            }, 
            Shutdown::Both => {
                let _ = self.close_write(None);
                self.close_read();
                owner.on_shutdown();
            }
        }
        Ok(())
    }

    fn clone_as_package_handler(&self) -> Option<Box<dyn OnPackage<SessionData>>> {
        None
    }

    fn clone_as_provider(&self)->Box<dyn StreamProvider> {
        Box::new(self.clone())
    }

    fn poll_readable(&self, cx: &mut Context<'_>) -> Poll<std::io::Result<usize>> {
        let read_proivder = &mut *cyfs_debug::lock!(self.0.read_provider).unwrap();
        match read_proivder {
            PollReadProvider::Open(recv_queue) => {
                recv_queue.readable(cx.waker())
            }, 
            PollReadProvider::Closed(r) => {
                match r {
                    Ok(len) => {
                        Poll::Ready(Ok(*len))
                    }, 
                    Err(err) => {
                        Poll::Ready(Err(std::io::Error::from(err.kind())))
                    }
                }
            }
        }
    }

    fn poll_read(
        &self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let read_proivder = &mut *cyfs_debug::lock!(self.0.read_provider).unwrap();
        let ret = match read_proivder {
            PollReadProvider::Open(recv_queue) => {
                let r = recv_queue.read(self, cx.waker(), buf);
                match &r {
                    Poll::Ready(r) => {
                        if let Ok(0) = r {
                            debug!("{} close recv queue for remote closed and no pending read", self);
                            *read_proivder = PollReadProvider::Closed(Ok(0));
                        }
                    }, 
                    _ => {}
                };
                r
            }, 
            PollReadProvider::Closed(r) => {
                match r {
                    Ok(len) => {
                        Poll::Ready(Ok(*len))
                    }, 
                    Err(err) => {
                        Poll::Ready(Err(std::io::Error::from(err.kind())))
                    }
                }
            }
        };
        ret
    }

    fn poll_write(
        &self,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if let Some(send_queue) = &mut *cyfs_debug::lock!(self.0.send_queue).unwrap() {
            let write_result = send_queue.write(self, buf, cx.waker());
            if let Some(to_send) = &write_result.buffer {
                let stream = self.clone();
                let (record_ptr, record_len) = stream.encode_record(to_send);
                task::spawn(async move {
                    stream.send_inner(record_ptr, record_len).await;
                });
            }

            if let Some(nagle_id) = write_result.nagle {
                let stream = self.clone();
                task::spawn(async move {
                    let _ = future::timeout(stream.0.config.nagle, future::pending::<()>()).await;
                    stream.on_nagle(nagle_id).await;
                });
            }
            write_result.poll
        } else {
            Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "send queue closing")))
        }
    }

    fn poll_flush(&self, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        if let Some(send_queue) = &mut *cyfs_debug::lock!(self.0.send_queue).unwrap() {
            send_queue.flush(cx.waker())
        } else {
            Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "send queue closing")))
        }
    }

    fn poll_close(&self, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.close_write(Some(cx.waker()))
    }
}
