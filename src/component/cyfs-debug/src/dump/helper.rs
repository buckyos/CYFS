use crate::DebugConfig;
use cyfs_base::*;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub struct DumpHelper {
    enable_dump: bool,
    full_dump: bool,
}

impl DumpHelper {
    fn new() -> Self {
        let enable_dump = match get_channel() {
            CyfsChannel::Nightly => true,
            _ => false,
        };

        Self {
            enable_dump,
            full_dump: false,
        }
    }

    pub fn get_instance() -> &'static Self {
        use once_cell::sync::OnceCell;
        static S_INSTANCE: OnceCell<DumpHelper> = OnceCell::new();
        S_INSTANCE.get_or_init(|| {
            let mut ret = Self::new();
            ret.load_config();
            ret
        })
    }

    pub fn is_enable_dump(&self) -> bool {
        self.enable_dump
    }

    fn default_basename() -> String {
        let arg0 = std::env::args().next().unwrap_or_else(|| "cyfs".to_owned());
        Path::new(&arg0).file_stem().map(OsStr::to_string_lossy).unwrap(/*cannot fail*/).to_string()
    }

    fn dump_file_name() -> String {
        let id = std::process::id();
        let now = chrono::Local::now();
        let now = now.format("%Y-%m-%d_%H-%M-%S%.6f_%z");

        format!("{}_{}_{}.dmp", Self::default_basename(), id, now)
    }

    fn dump_dir() -> PathBuf {
        let dump_dir = cyfs_util::get_log_dir("core-dump");
        if !dump_dir.is_dir() {
            if let Err(e) = std::fs::create_dir_all(&dump_dir) {
                error!(
                    "create core-dump dir failed! dir={}, err={}",
                    dump_dir.display(),
                    e
                );
            }
        }

        dump_dir
    }

    fn load_config(&mut self) {
        if let Some(config_node) = DebugConfig::get_config("dump") {
            if let Err(e) = self.load_config_value(config_node) {
                println!("load process dead check config error! {}", e);
            }
        }
    }

    fn load_config_value(&mut self, config_node: &toml::Value) -> BuckyResult<()> {
        let node = config_node.as_table().ok_or_else(|| {
            let msg = format!("invalid dump config format! content={}", config_node,);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        for (k, v) in node {
            match k.as_str() {
                "enable" => {
                    if let Some(v) = v.as_bool() {
                        println!(
                            "load dump.enable from config: {}, current={}",
                            v, self.enable_dump
                        );
                        self.enable_dump = v;
                    } else {
                        println!("unknown dump.enable config node: {:?}", v);
                    }
                }
                "full" => {
                    if let Some(v) = v.as_bool() {
                        println!(
                            "load dump.full from config: {}, current={}",
                            v, self.full_dump
                        );
                        self.enable_dump = v;
                    } else {
                        println!("unknown dump.full config node: {:?}", v);
                    }
                }

                key @ _ => {
                    println!("unknown dump config node: {}={:?}", key, v);
                }
            }
        }

        Ok(())
    }

    pub fn dump(&self) {
        let dir = Self::dump_dir();
        let filename = Self::dump_file_name();

        info!("will create dump file: {}/{}", dir.display(), filename);

        super::dump::create_dump(&dir, &filename, self.full_dump)
    }
}
