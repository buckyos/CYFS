use crate::DebugConfig;
use cyfs_base::*;

use once_cell::sync::OnceCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;


#[derive(Clone)]
pub struct ProcessDeadHelper {
    interval_in_secs: u64,

    task_system_last_active: Arc<AtomicU64>,

    exit_on_task_system_dead: Arc<AtomicU64>,

    exit_on_dead: bool,
}

impl ProcessDeadHelper {
    fn new(interval_in_secs: u64) -> Self {
        let exit_on_dead = match get_channel() {
            CyfsChannel::Nightly => false,
            _ => true,
        };

        let mut ret = Self {
            interval_in_secs,
            task_system_last_active: Arc::new(AtomicU64::new(bucky_time_now())),
            exit_on_task_system_dead: Arc::new(AtomicU64::new(0)),
            exit_on_dead,
        };

        ret.load_config();
        ret
    }

    fn load_config(&mut self) {
        if let Some(config_node) = DebugConfig::get_config("check") {
            if let Err(e) = self.load_config_value(config_node) {
                println!("load process dead check config error! {}", e);
            }
        }
    }

    fn load_config_value(&mut self, config_node: &toml::Value) -> BuckyResult<()> {
        let node = config_node.as_table().ok_or_else(|| {
            let msg = format!("invalid debug config format! content={}", config_node,);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        for (k, v) in node {
            match k.as_str() {
                "exit_on_dead" => {
                    if let Some(v) = v.as_bool() {
                        println!("load check.exit_on_dead from config: {}, current={}", v, self.exit_on_dead);
                        self.exit_on_dead = v;
                    } else {
                        println!("unknown exit_on_dead config node: {:?}", v);
                    }
                }

                key @ _ => {
                    println!("unknown check config node: {}={:?}", key, v);
                }
            }
        }

        Ok(())
    }

    pub fn patch_task_min_thread() {
        let cpu_nums = num_cpus::get();
        if cpu_nums <= 1 {
            const KEY: &str = "ASYNC_STD_THREAD_COUNT";
            if std::env::var(KEY).is_err() {
                std::env::set_var(KEY, "2");
            }
        }
    }

    pub fn instance() -> &'static Self {
        static INSTANCE: OnceCell<ProcessDeadHelper> = OnceCell::new();
        INSTANCE.get_or_init(|| Self::new(60))
    }

    pub fn start_check(&self) {
        static INIT_DONE: AtomicBool = AtomicBool::new(false);
        if !INIT_DONE.swap(true, Ordering::SeqCst) {
            self.start_check_process();
            self.start_check_task_system();
        }
    }

    pub fn enable_exit_on_task_system_dead(&self, timeout_in_secs: Option<u64>) {
        let v = timeout_in_secs.unwrap_or(60 * 5) * 1000 * 1000;
        self.exit_on_task_system_dead.store(v, Ordering::SeqCst);
        if v > 0 {
            info!("enable exit on task system dead: timeout={}", v);
            self.start_check();
        } else {
            info!("disable exit on task system dead");
        }
    }

    fn update_task_alive(&self) {
        let now = bucky_time_now();
        self.task_system_last_active.store(now, Ordering::SeqCst);
    }

    fn check_task_alive(&self) {
        let exit_timeout = self.exit_on_task_system_dead.load(Ordering::SeqCst);
        if exit_timeout == 0 || !self.exit_on_dead {
            return;
        }

        let now = bucky_time_now();
        let last_active = self.task_system_last_active.load(Ordering::SeqCst);
        if now >= last_active && now - last_active >= exit_timeout {
            error!(
                "task system dead timeout, now will exit process! last_active={}, exit_timeout={}s",
                last_active,
                exit_timeout / (1000 * 1000)
            );
            println!("process will exit on task system dead...");

            let ins = crate::dump::DumpHelper::get_instance();
            if ins.is_enable_dump() {
                ins.dump();
            }
 
            std::thread::sleep(std::time::Duration::from_secs(5));
            std::process::exit(-1);
        }
    }

    fn start_check_process(&self) {
        let dur = std::time::Duration::from_secs(self.interval_in_secs);

        let this = self.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(dur);
            info!("process still alive {:?}, {}", std::thread::current().id(), cyfs_base::get_version());
            this.check_task_alive();
        });
    }

    fn start_check_task_system(&self) {
        let dur = std::time::Duration::from_secs(self.interval_in_secs);
        let this = self.clone();
        async_std::task::spawn(async move {
            loop {
                this.update_task_alive();
                async_std::task::sleep(dur).await;
                info!(
                    "process task system still alive {:?}",
                    std::thread::current().id(),
                );
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use cyfs_base::bucky_time_to_system_time;

    use super::ProcessDeadHelper;
    use std::sync::RwLock;

    struct Test {
        v: Option<u32>,
    }

    impl Test {
        fn new() -> Self {
            Self { v: None }
        }

        fn get(&self) -> Option<&u32> {
            self.v.as_ref()
        }

        fn set(&mut self, v: u32) {
            self.v = Some(v);
        }
    }

    async fn dead_lock() {
        let r: RwLock<Test> = RwLock::new(Test::new());

        if let Some(v) = r.read().unwrap().get().cloned() {
            println!("v={}", v);
        } else {
            println!("enter else");
            r.write().unwrap().set(1);
            println!("v={}", 1);
        };
    }

    #[test]
    fn test_time() {
        let t = 13316567010962630;
        let s = bucky_time_to_system_time(t);
        println!("{:#?}", s);

        let datetime: chrono::DateTime<chrono::Local> = s.into();
        let time_str = datetime.format("%Y-%m-%d %H:%M:%S%.3f %:z");
        println!("{}", time_str);
    }

    #[test]
    fn test_dead_lock() {
        ProcessDeadHelper::instance().start_check();
        ProcessDeadHelper::instance().enable_exit_on_task_system_dead(Some(1000 * 1000 * 2));

        async_std::task::block_on(dead_lock());

        // async_std::task::sleep(std::time::Duration::from_secs(60 * 5));
    }

    #[test]
    fn test_safe_lock() {
        let r: RwLock<Test> = RwLock::new(Test::new());

        let v = r.read().unwrap().get().cloned();
        if let Some(v) = v {
            println!("v={}", v);
        } else {
            println!("enter else");
            r.write().unwrap().set(1);
            println!("v={}", 1);
        };
    }
}
