use log::*;
use std::{
    time::{Duration},
    collections::LinkedList, 
    task::{Poll, Waker},
    sync::Mutex
};
use cyfs_base::*;
// use cyfs_debug::Mutex;
use crate::{
    types::*, 
    protocol::{*, v0::*}, 
    cc::*
};
use super::{
    send_queue::SendQueue, 
    stream::PackageStream, 
};

struct EstimateStub {
    pub id: IncreaseId, 
    pub send_time: Timestamp, 
    pub end_pos: u64
}


struct WriteProviderImpl {
    write_waiter: Option<Waker>, 
    flush_waiters: LinkedList<Waker>, 
    close_waiter: Option<Waker>, 
    queue: SendQueue,
    est_stubs: LinkedList<EstimateStub>, 
    est_id: IncreaseIdGenerator, 
    last_recv: Timestamp, 
    cc: CongestionControl
}

impl WriteProviderImpl {
    fn check_wnd(&mut self, stream: &PackageStream, now: Timestamp, timeout: Duration, packages: &mut Vec<DynamicPackage>, logging: bool) {
        self.queue.check_wnd(stream, now, timeout, self.cc.cwnd(), packages, logging);
        self.on_pre_send_package(stream, packages);
    }

    fn on_time_escape(&mut self, stream: &PackageStream, now: Timestamp, packages: &mut Vec<DynamicPackage>) -> BuckyResult<()> {
        let timeout = self.cc.rto();
        let nagle = self.queue.check_nagle(stream, now);
        let (lost, _break) = if self.queue.flight() > 0 {
            if self.queue.check_timeout(now, self.cc.rto()) {
                debug!("{} ledbat lost some package's ack", stream);
                self.cc.on_loss(0);
                (true, false)
            } else if now > self.last_recv {
                let d = Duration::from_micros(now - self.last_recv);
                if d > stream.config().package.break_overtime {
                    (true, true)
                } else if d >= self.cc.rto() {
                    debug!("{} ledbat no ack in rto", stream);
                    self.cc.on_no_resp(0);
                    (true, false)
                } else {
                    (false, false)
                }
            } else {
                (false, false)
            }
        } else {
            (false, false)
        };

        if _break {
            error!("{} break write for no ack", stream);
            Err(BuckyError::new(BuckyErrorCode::ErrorState, "write break"))
        } else {
            if nagle || lost {
                self.check_wnd(stream, now, timeout, packages, lost)
            }
            Ok(())
        }
    }

    fn on_pre_send_package(&mut self, stream: &PackageStream, packages: &mut Vec<DynamicPackage>) {
        // 选取评估包，加上package id字段
        let last_sample = self.est_stubs.back();
        let last_sample_time = self.est_stubs.back().map_or(0, |sample| sample.send_time);
        for session_data in packages.iter_mut().map(|p| AsMut::<SessionData>::as_mut(p)) {
            if last_sample.is_none() || 
                (session_data.stream_pos_end() >= last_sample.unwrap().end_pos 
                    && session_data.send_time >= last_sample_time
                    && Duration::from_micros(session_data.send_time - last_sample_time) > self.cc.rtt()) {
                let id = self.est_id.generate();
                session_data.flags_add(SESSIONDATA_FLAG_PACKAGEID);
                session_data.id_part = Some(SessionDataPackageIdPart {
                    package_id: id,
                    total_recv: 0,
                });
                trace!("{} will send estimate package {}", stream, session_data);
                self.est_stubs.push_back(EstimateStub {
                    id,
                    send_time: session_data.send_time,
                    end_pos: session_data.stream_pos_end()
                });
                break;
            }
        }
    }
}

enum WriteProviderState {
    Open(WriteProviderImpl), 
    Closed
}

pub struct WriteProvider(Mutex<WriteProviderState>);

impl WriteProvider {
    pub fn new(config: &super::super::container::Config) -> Self {
        Self(Mutex::new(WriteProviderState::Open(WriteProviderImpl {
            write_waiter: None, 
            flush_waiters: LinkedList::new(), 
            close_waiter: None, 
            queue: SendQueue::new(config.send_buffer), 
            est_id: IncreaseIdGenerator::new(), 
            est_stubs: LinkedList::new(), 
            last_recv: bucky_time_now(), 
            cc: CongestionControl::new(PackageStream::mss(), &config.package.cc)
        })))
    }

    pub fn on_time_escape(&self, stream: &PackageStream, now: Timestamp, packages: &mut Vec<DynamicPackage>) -> BuckyResult<BuckyResult<()>> {
        let (waiters, ret) = {
            let state = &mut *cyfs_debug::lock!(self.0).unwrap();
            match state {
                WriteProviderState::Open(provider) => {
                    let ret = provider.on_time_escape(stream, now, packages);
                    if ret.is_err() {
                        let mut waiters = LinkedList::new();
                        if let Some(waiter) = &provider.write_waiter {
                            waiters.push_back(waiter.clone());
                        }
                        if let Some(waiter) = &provider.close_waiter {
                            waiters.push_back(waiter.clone());
                        }
                        waiters.append(&mut provider.flush_waiters);
                        *state = WriteProviderState::Closed;
                        (Some(waiters), Err(ret.unwrap_err()))
                    } else {
                        (None, Ok(ret))
                    }
                }, 
                WriteProviderState::Closed => (None, Ok(Err(BuckyError::new(BuckyErrorCode::ErrorState, "write closed"))))
            }
        };
        if let Some(waiters) = waiters {
            for w in waiters {
                w.wake();
            }
        }
        ret
    }

    pub fn write(&self, stream: &PackageStream, waker: &Waker, buf: &[u8]) -> Poll<std::io::Result<usize>> {
        let mut packages = Vec::new();
        let result = {
            let state = &mut *cyfs_debug::lock!(self.0).unwrap();
            match state {
                WriteProviderState::Open(provider) => {
                    if provider.write_waiter.is_some() {
                        let msg = format!("{} pending write for former pending write", stream);
                        error!("{}", msg.as_str());
                        return Poll::Pending;
                    } else {
                        let used = provider.queue.used();
                        let writen = provider.queue.alloc_blocks(stream, buf);
                        if writen == 0 {
                            provider.write_waiter = Some(waker.clone());
                            Ok(0)
                        } else {
                            // 如果send queue 为空， 第一次write的时候更新last recv
                            if used > 0 {
                                provider.last_recv = bucky_time_now();
                            }
                            provider.check_wnd(stream, bucky_time_now(), provider.cc.rto(), &mut packages, false);
                            Ok(writen)
                        }
                    }
                }, 
                WriteProviderState::Closed => Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "write closed")), 
            }
        };
        match result {
            Ok(writen) => {
                let _ = stream.send_packages(packages);
                if writen == 0 {
                    debug!("{} write {} bytes pending", stream, buf.len());
                    Poll::Pending
                } else {
                    debug!("{} write {} bytes {} writen", stream, buf.len(), writen);
                    Poll::Ready(Ok(writen))
                }
            }, 
            Err(e) => {
                Poll::Ready(Err(e))
            }
        }
    }

    pub fn flush(&self, waker: &Waker) -> Poll<std::io::Result<()>> {
        let state = &mut *cyfs_debug::lock!(self.0).unwrap();
        match state {
            WriteProviderState::Open(provider) => {
                if provider.queue.used() == 0 {
                    Poll::Ready(Ok(()))
                } else {
                    provider.flush_waiters.push_back(waker.clone());
                    Poll::Pending
                }
            }, 
            WriteProviderState::Closed => Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "write closed")))
        }   
    }
    
    pub fn close(&self, stream: &PackageStream, waker: Option<&Waker>) -> Poll<std::io::Result<()>> {
        let mut packages = Vec::new();
        let result = {
            let state = &mut *cyfs_debug::lock!(self.0).unwrap();
            match state {
                WriteProviderState::Open(provider) => {
                    provider.queue.close(stream);
                    provider.close_waiter = waker.map(|w| w.clone());
                    provider.check_wnd(stream, bucky_time_now(), provider.cc.rto(), &mut packages, false);
                    Poll::Pending
                }, 
                WriteProviderState::Closed => Poll::Ready(Ok(()))
            }   
        };
        let _ = stream.send_packages(packages);
        result
    }
}

impl OnPackage<SessionData, (&PackageStream, &mut Vec<DynamicPackage>)> for WriteProvider {
    fn on_package(&self, session_data: &SessionData, context: (&PackageStream, &mut Vec<DynamicPackage>)) -> Result<OnPackageResult, BuckyError> {
        let stream = context.0;
        let mut packages = context.1;
        let (result, waiters) = {
            let state = &mut *cyfs_debug::lock!(self.0).unwrap();
            match state {
                WriteProviderState::Open(provider) => {
                    let now = bucky_time_now();
                    if session_data.is_flags_contain(SESSIONDATA_FLAG_RESET) {
                        *state = WriteProviderState::Closed;
                        return Ok(OnPackageResult::Handled)
                    } else if session_data.is_flags_contain(SESSIONDATA_FLAG_ACK_PACKAGEID) {
                        let ack_est_package = session_data;
                        let package_id = ack_est_package.id_part.as_ref().unwrap().package_id;
                        trace!("{} recv estimate ack package {}", stream, package_id);
                        let mut to_remove = None;
                        for (index, sample) in (&provider.est_stubs).iter().enumerate() {
                            if sample.id == package_id {
                                let rtt = Duration::from_micros(bucky_time_now() - sample.send_time);
                                let delay = rtt / 2;
                                provider.cc.on_estimate(rtt, delay);
                                debug!("{} estimate rtt:{:?} delay:{:?} rto:{:?}", stream, rtt, delay, provider.cc.rto());
                                to_remove = Some(index);
                                break;  
                            } 
                        }
                        if let Some(index) = to_remove {
                            provider.est_stubs = provider.est_stubs.split_off(index + 1);
                        }
                        (Ok(OnPackageResult::Handled), None)
                    } else {
                        let mut waiters = LinkedList::new();
                        if session_data.is_flags_contain(SESSIONDATA_FLAG_PACKAGEID) {
                            let est_package = session_data;
                            let mut ack_est_package = SessionData::new();
                            ack_est_package.flags_add(SESSIONDATA_FLAG_ACK_PACKAGEID);
                            ack_est_package.id_part = Some(SessionDataPackageIdPart {
                                package_id: est_package.id_part.as_ref().unwrap().package_id,
                                total_recv: 0,
                            });
                            ack_est_package.send_time = now;
                            trace!("{} ack estimate {} ", stream, ack_est_package);
                            packages.push(DynamicPackage::from(ack_est_package));
                        }
                        let (newly_acked, fin) = provider.queue.confirm(stream, session_data.ack_stream_pos, session_data.is_flags_contain(SESSIONDATA_FLAG_FINACK));
                        provider.last_recv = now;
                        if newly_acked > 0 {
                            trace!("{} newly ack {} to {}", stream, newly_acked, provider.queue.start());
                            provider.cc.on_ack(provider.queue.flight() as u64, 
                            newly_acked as u64, 
                            Some(newly_acked as u64), 
                            bucky_time_now());
                            debug!("{} update cwnd: {}", stream, provider.cc.cwnd());
                            provider.check_wnd(stream, now, provider.cc.rto(), &mut packages, false);
                            
                            if provider.queue.used() < stream.config().send_drain() {
                                if let Some(waiter) = provider.write_waiter.as_ref() {
                                    waiters.push_back(waiter.clone());
                                    provider.write_waiter = None;
                                    debug!("{} send queue wake write waiter", stream);
                                } 
                            }
                            if provider.queue.used() == 0 {
                                trace!("{} send queue empty", stream);
                                waiters.append(&mut provider.flush_waiters);
                                debug!("{} send queue wake flush waiter", stream); 
                            }
                        }
                        if fin {
                            debug!("{} send queue got fin ack, enter Closed", stream);
                            if let Some(waiter) = provider.close_waiter.as_ref() {
                                waiters.push_back(waiter.clone());
                                provider.close_waiter = None;
                            }
                            *state = WriteProviderState::Closed;
                        }
                        (Ok(OnPackageResult::Continue), Some(waiters))
                    }
                }, 
                WriteProviderState::Closed => {
                    debug!("closed stream get data");

                    (Ok(OnPackageResult::Break), None)
                }
            }
        };
        
        if let Some(waiters) = waiters {
            for waiter in waiters {
                waiter.wake();
            }
        }
        result
    }
}
