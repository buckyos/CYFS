use log::*;
use std::{
    time::Duration, 
    collections::LinkedList, 
    sync::Mutex
};
// use cyfs_debug::{Mutex};
use async_std::{
    task, 
    future, 
    task::{Poll, Waker},
};
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::{*, v0::*}
};
use super::{
    recv_queue::RecvQueue, 
    stream::PackageStream,
};


struct ReadStub {
    waker: Waker, 
    len: usize, 
    time: Timestamp
}

enum NagleState {
    None, 
    Nagle(Timestamp/*latest recv*/)
}


enum ReadProviderState {
    Open(ReadProviderImpl), 
    Closed(u64, /*last ack pos*/std::io::Result<usize>/*last result*/), 
}

struct ReadProviderImpl {
    // wait 2*msl
    remote_closed: Option<Timestamp>,
    timeout: bool, 
    read_waiter: Option<ReadStub>, 
    readable_waiter: Option<Waker>, 
    queue: RecvQueue, 
    nagle: NagleState
}

impl ReadProviderImpl {
    fn check_close_waiting(&self, now: Timestamp, msl: Duration) -> bool {
        if let Some(when) = self.remote_closed.as_ref() {
            //FIXME: 2 * msl
            if self.queue.stream_len() == 0 
                && now >= *when 
                && Duration::from_micros(now - *when) > 2 * msl {
                return true;
            }  
        }
        false
    }

    fn check_timeout(&mut self, stream: &PackageStream, stub_time: u64) -> Option<Waker> {
        trace!("{} checking timeout of read at {}", stream, stub_time);
        if self.timeout {
            trace!("{} checking timeout of read at {} has another timeout", stream, stub_time);
            return None;
        }
        if self.remote_closed.is_some() {
            trace!("{} checking timeout of read at {} remote closed", stream, stub_time);
            return None;
        }
        if self.read_waiter.is_none() {
            trace!("{} checking timeout of read at {} waiter has removed", stream, stub_time);
            return None;
        }
        let stub = self.read_waiter.as_ref().unwrap();
        if self.queue.stream_len() == 0 {
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

    fn readable(&mut self, waker: &Waker) -> Poll<std::io::Result<usize>> {
        if self.queue.stream_len() > 0 {
            Poll::Ready(Ok(self.queue.stream_len()))
        } else {
            assert!(self.read_waiter.is_none());
            self.readable_waiter = Some(waker.clone());
            Poll::Pending
        }
    }

    fn read(&mut self, stream: &PackageStream, waker: &Waker, buf: &mut [u8]) -> (Poll<std::io::Result<usize>>, Option<Timestamp>) {
        if self.read_waiter.is_some() {
            let msg = format!("{} pending read for former pending read", stream);
            error!("{}", msg.as_str());
            return (Poll::Pending, None);
        }
        let former_len = self.queue.stream_len();
        let config = stream.config();
        if former_len >= buf.len() {
            debug!("{} read {} bytes ready for buffer enough", stream, buf.len());
            self.timeout = false;
        } else if former_len > config.recv_drain() {
            debug!("{} read {} bytes ready for buffer greater than drain", stream, buf.len());
            self.timeout = false;
        } else if self.remote_closed.is_some() {
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
            debug!("{} pending read at {} for no enough buffer {}", stream, stub_time, former_len);
            return (Poll::Pending, Some(stub_time));
        }

        let recv_len = self.queue.read_stream(buf);

        debug!("{} poll read return {} bytes", stream, recv_len);
        (Poll::Ready(Ok(recv_len)), None)
    }

    fn wake(&mut self, stream: &PackageStream, total: usize) -> Option<Waker> {
        let now = bucky_time_now();
        let mut to_wake = None;
        if self.remote_closed.is_some() {
            to_wake = self.read_waiter.as_ref().map(|stub| stub.waker.clone());
            self.read_waiter = None;
        } else if let Some(stub) = self.read_waiter.as_ref() {
            if total >= stub.len {
                debug!("{} wake read at {} {} bytes for enough buffer", stream, stub.time, stub.len);
                to_wake = Some(stub.waker.clone());
                self.read_waiter = None;
            } else if (now >= stub.time) && (Duration::from_micros(now - stub.time) > stream.config().recv_timeout) {
                debug!("{} wake read at {} {} bytes for timeout", stream, stub.time, stub.len);
                self.timeout = true;
                to_wake = Some(stub.waker.clone());
                self.read_waiter = None;
            } else if total > stream.config().recv_drain() {
                debug!("{} wake read at {} {} bytes for drain", stream, stub.time, stub.len);
                to_wake = Some(stub.waker.clone());
                self.read_waiter = None;
            }
        }
        
        to_wake
    }
}

pub struct ReadProvider(Mutex<ReadProviderState>);

impl ReadProvider {
    pub fn new(config: &super::super::container::Config) -> Self {
        Self(Mutex::new(
            ReadProviderState::Open(ReadProviderImpl {
                remote_closed: None, 
                timeout: false, 
                read_waiter: None, 
                readable_waiter: None, 
                queue: RecvQueue::new(config.recv_buffer), 
                // 初始化 NagleState 为 nagle，保证在不send的时候，也会回复第一个ack作为ackack
                nagle: NagleState::Nagle(bucky_time_now())
            })))
    }

    pub fn touch_ack(&self, _stream: &PackageStream) -> (u64, bool) {
        let state = &mut *(cyfs_debug::lock!(self.0)).unwrap();
        
        let ret = match state {
            ReadProviderState::Open(provider) => {
                match &provider.nagle {
                    NagleState::Nagle(_) => {
                        provider.nagle = NagleState::None;
                    }, 
                    NagleState::None => {
        
                    }
                };
                (provider.queue.stream_end(), provider.remote_closed.is_some())
            },
            ReadProviderState::Closed(last_ack, _) => {
                (*last_ack, true)
            }
        };
        ret
    } 

    pub fn on_time_escape(&self, stream: &PackageStream, now: Timestamp, packages: &mut Vec<DynamicPackage>) -> BuckyResult<()> {
        let state = &mut *cyfs_debug::lock!(self.0).unwrap();

        let ret = match state {
            ReadProviderState::Open(provider) => {
                match &provider.nagle {
                    NagleState::Nagle(when) => {
                        if now > *when && Duration::from_micros(now - *when) > stream.config().nagle {
                            trace!("{} will ack for nagle timeout", stream);
                            let mut package = SessionData::new();
                            package.flags_add(SESSIONDATA_FLAG_ACK);
                            package.send_time = now;
                            packages.push(DynamicPackage::from(package));
                        }
                    },
                    NagleState::None => {
                        // do nothing
                    }
                };
                if provider.check_close_waiting(now, stream.config().package.msl) {
                    *state = ReadProviderState::Closed(provider.queue.stream_end(), Ok(0));
                }
                Ok(())
            },
            ReadProviderState::Closed(_, _) => {
                Err(BuckyError::new(BuckyErrorCode::ErrorState, "read closed"))
            }
        };
        ret
    }

    pub fn break_with_error(&self, _err: BuckyError) {
        let to_wake = {
            let mut to_wake = LinkedList::new();
            let state = &mut *cyfs_debug::lock!(self.0).unwrap();
            match state {
                ReadProviderState::Open(provider) => {
                    if let Some(stub) = provider.read_waiter.as_ref() {
                        to_wake.push_back(stub.waker.clone());
                    } 
                    if let Some(readable) = provider.readable_waiter.as_ref() {
                        to_wake.push_back(readable.clone());
                    }
                    *state = ReadProviderState::Closed(provider.queue.stream_end(), Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "stream broken")));
                }, 
                ReadProviderState::Closed(_, _) => {
                    
                }
            }
            to_wake
        };
        
        for waker in to_wake {
            waker.wake()
        }
    }

    pub fn readable(&self, waker: &Waker) -> Poll<std::io::Result<usize>> {
        let state = &mut *cyfs_debug::lock!(self.0).unwrap();
        match state {
            ReadProviderState::Open(provider) => {
                provider.readable(waker)
            }, 
            ReadProviderState::Closed(_, result) => {
                match result {
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

    pub fn read(&self, stream: &PackageStream, waker: &Waker, buf: &mut [u8]) -> Poll<std::io::Result<usize>> {
        let (ret, check_timeout) = {
            let state = &mut *cyfs_debug::lock!(self.0).unwrap();

            let (ret, check_timeout) = match state {
                ReadProviderState::Open(provider) => {
                    let (ret, check_timeout) = provider.read(stream, waker, buf);
                    match &ret {
                        Poll::Ready(r) => {
                            if let Ok(0) = r {
                                debug!("{} close recv queue for remote closed and no pending read", stream);
                                if provider.check_close_waiting(bucky_time_now(), stream.config().package.msl) {
                                    *state = ReadProviderState::Closed(provider.queue.stream_end(), Ok(0));
                                }
                            }
                        }, 
                        _ => {}
                    };
                    (ret, check_timeout)
                }, 
                ReadProviderState::Closed(_, result) => {
                    match result {
                        Ok(len) => {
                            (Poll::Ready(Ok(*len)), None)
                        }, 
                        Err(err) => {
                            (Poll::Ready(Err(std::io::Error::from(err.kind()))), None)
                        }
                    }
                }
            };

            (ret, check_timeout)
        };
        
        if let Some(stub_time) = check_timeout {
            let stream = stream.clone();
            task::spawn(async move {
                let to_wake = {
                    {
                        cyfs_debug::scope_tracker!(std::time::Duration::from_secs(5));
                        let _ = future::timeout(stream.config().recv_timeout, future::pending::<()>()).await;
                    }

                    let read_provider = &mut *cyfs_debug::lock!(stream.read_provider().0).unwrap();

                    let to_wake = match read_provider {
                        ReadProviderState::Open(provider) => {
                            provider.check_timeout(&stream, stub_time)
                        }, 
                        ReadProviderState::Closed(_, _) => {
                            None
                        }
                    };
                    to_wake
                };
                if let Some(to_wake) = to_wake {
                    to_wake.wake();
                }
            });
        }

        ret
    }
}

impl OnPackage<SessionData, (&PackageStream, &mut Vec<DynamicPackage>)> for ReadProvider {
    fn on_package(&self, session_data: &SessionData, context: (&PackageStream, &mut Vec<DynamicPackage>)) -> Result<OnPackageResult, BuckyError> {
        let stream = context.0;
        let packages = context.1;
        let (readable_waker, read_waker) = {
            let state = &mut *cyfs_debug::lock!(self.0).unwrap();
            match state {
                ReadProviderState::Open(provider) => {
                    if session_data.payload.as_ref().len() > 0 
                        || session_data.is_flags_contain(SESSIONDATA_FLAG_FIN) {
                        match &provider.nagle {
                            NagleState::None => {
                                trace!("{} no ack on recv data for waiting nagle", stream);
                                provider.nagle = NagleState::Nagle(bucky_time_now())
                            },
                            NagleState::Nagle(_) => {
                                // do nothing
                            }
                        };
                    }
                    if provider.remote_closed.is_none() {
                        let now = bucky_time_now();
                        let (confirmed, fin) = provider.queue.push(stream, session_data);
                        if fin {
                            provider.remote_closed = Some(now);
                        }
                        if (confirmed > 0 && session_data.payload.as_ref().len() < PackageStream::mss()) || fin {
                            if packages.len() == 0 || !(&packages[packages.len() - 1] as &dyn AsRef<SessionData>).as_ref().is_flags_contain(SESSIONDATA_FLAG_ACK) {
                                trace!("{} will ack for fin or nagle income", stream);
                                let mut package = SessionData::new();
                                package.flags_add(SESSIONDATA_FLAG_ACK);
                                package.send_time = now;
                                packages.push(DynamicPackage::from(package));
                            }
                        }
                        if confirmed > 0 || fin {
                            let mut readable_waker = None;
                            std::mem::swap(&mut provider.readable_waiter, &mut readable_waker);
                            (readable_waker, provider.wake(stream, provider.queue.stream_len()))
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    }
                   
                }, 
                ReadProviderState::Closed(_, _) => {
                    // do nothing
                    (None, None)
                }
            }
        };
        if let Some(to_wake) = readable_waker {
            to_wake.wake();
        }
        if let Some(to_wake) = read_waker {
            to_wake.wake();
        }
        Ok(OnPackageResult::Handled)
    }
}




#[test]
fn profile_debug_mutex() {
    for i in 0..10000 {
        use cyfs_debug::Mutex;
        let m = Mutex::new(i);
        *cyfs_debug::lock!(m).unwrap() = 0;
    }
}


#[test]
fn profile_mutex() {
    for i in 0..10000 {
        use std::sync::Mutex;
        let m = Mutex::new(i);
        *m.lock().unwrap() = 0;
    }
}