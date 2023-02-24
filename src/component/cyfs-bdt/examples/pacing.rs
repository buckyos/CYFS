use std::{
    sync::{atomic::{AtomicU64, Ordering, AtomicBool}, Mutex, Arc},
    time::{Instant, Duration},
    collections::LinkedList,
    thread,
};

use cyfs_base::*;
use cyfs_bdt::{cc::pacing};

struct PacePackage {
    send_time: Instant,
    package_size: u64,
}
struct PackageStreamMockImpl {
    pacer: Mutex<pacing::Pacer>, 
    package_queue: Arc<Mutex<LinkedList<PacePackage>>>, 
    sent_bytes: AtomicU64,
    finish: AtomicBool,
    end: Mutex<Option<Instant>>,
    thread: Mutex<Option<thread::JoinHandle<bool>>>,
}

#[derive(Clone)]
pub struct PackageStreamMock(Arc<PackageStreamMockImpl>);

fn mss() -> usize {
    1472
}

impl PackageStreamMock {
    pub fn new(rate: u64) -> BuckyResult<Self> {
        let mut pacer = pacing::Pacer::new(true, mss()*4, mss());
        pacer.update(rate);

        let stream = Self(Arc::new(PackageStreamMockImpl {
            pacer: Mutex::new(pacer),
            package_queue: Arc::new(Mutex::new(Default::default())),
            sent_bytes: AtomicU64::new(0),
            finish: AtomicBool::new(false),
            end: Mutex::new(None),
            thread: Mutex::new(None),
        }));

        Ok(stream)
    }

    pub fn send_packages(&self, packages: Vec<u64>) -> Result<(), BuckyError> {
        let mut pacer = self.0.pacer.lock().unwrap();
        let now = Instant::now();
        let mut sent_bytes = 0;
        for package in packages {
            if let Some(next_time) = pacer.send(package as usize, now) {
                self.package_delay(package, next_time);
            } else {
                sent_bytes += package;
            }
        }
        self.0.sent_bytes.fetch_add(sent_bytes, Ordering::SeqCst);

        Ok(())
    }

    pub fn package_delay(&self, package_size: u64, send_time: Instant) {
        let mut package_queue = self.0.package_queue.lock().unwrap();
        package_queue.push_back(PacePackage {
            send_time,
            package_size,
        });

        if package_queue.len() == 1 {
            let mut delay = Instant::now() - send_time;
            let package_queue = self.0.package_queue.clone();
            let stream = self.clone();
            let t = thread::spawn(move || {
                loop {
                    thread::sleep(delay);

                    let now = Instant::now();
                    {
                        let mut packages = package_queue.lock().unwrap();
                        let mut n = 0;

                        for (_, package) in packages.iter().enumerate() {
                            if package.send_time > now {
                                delay = package.send_time.checked_duration_since(now).unwrap();
                                break ;
                            }
                            n += 1;
                        }

                        let mut sent_bytes = 0;
                        while n > 0 {
                            if let Some(package) = packages.pop_front() {
                                sent_bytes += package.package_size;
                            }
                            n -= 1;
                        }
                        stream.0.sent_bytes.fetch_add(sent_bytes, Ordering::SeqCst);

                        if packages.len() == 0 {
                            if stream.0.finish.load(Ordering::SeqCst) {
                                let mut end = stream.0.end.lock().unwrap();
                                *end = Some(Instant::now());
                            }
                            return true;
                        }
                    }
                }
            });

            let mut thread = self.0.thread.lock().unwrap();
            *thread = Some(t);
        }
    }
}

fn packages_gen(n: usize) -> Vec<u64> {
    let mut packages = vec![0u64];

    let mut n = n;
    loop {
        if n == 0 {
            break;
        }
        n -= 1;

        packages.push(mss() as u64);
    }

    packages
}

fn pacing_test(data_size: usize, rate: u64) {
    println!("data_size={} | {} MB, rate={} | {} KB/s", data_size,  data_size/1024/1024, rate, rate/1024);

    let n = 1000;

    let stream = PackageStreamMock::new(rate).unwrap();

    let mut num = (data_size / mss() + 1) / n + 1;

    let start = Instant::now();
    loop {
        let _ = stream.send_packages(packages_gen(n));
        std::thread::sleep(Duration::from_millis(20));

        num -= 1;
        if num == 0 {
            stream.0.finish.store(true, Ordering::SeqCst);
            break ;
        }
    }

    {
        let t = stream.0.thread.lock().unwrap().take();
        if let Some(thread) = t {
            let _ = thread.join();
        }
    }

    let end = (*stream.0.end.lock().unwrap()).unwrap();
    let cost = end.checked_duration_since(start).unwrap();
    let sent_bytes = stream.0.sent_bytes.load(Ordering::SeqCst);

    println!("cost={:?} sent_bytes={} rate={:.1} KB/s", 
        cost, sent_bytes, sent_bytes as f64 / 1024.0 / cost.as_secs_f64());
}

#[async_std::main]
async fn main() {
    pacing_test(1024*1024*30, 1024*1024*3);
    pacing_test(1024*1024*30, 1024*1024*7);
    pacing_test(1024*1024*50, 1024*1024*9);
}