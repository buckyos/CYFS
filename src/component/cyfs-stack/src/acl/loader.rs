use super::config::*;
use super::table::AclTableContainer;
use cyfs_base::*;

use std::path::PathBuf;
use toml::Value as Toml;

#[derive(Clone)]
pub(crate) struct AclFileLoader {
    root: PathBuf,
}

impl AclFileLoader {
    pub fn new(config_isolate: Option<&String>) -> Self {
        let mut root = cyfs_util::get_cyfs_root_path();
        root.push("etc");
        if let Some(isolate) = config_isolate {
            if isolate.len() > 0 {
                root.push(isolate.as_str());
            }
        }
        root.push("acl");

        Self { root }
    }

    pub fn load_file(&self, file_name: &str) -> BuckyResult<Toml> {
        let file = self.root.join(file_name);
        if !file.is_file() {
            let msg = format!("acl config file not found: {}", file.display());
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let content = std::fs::read_to_string(&file).map_err(|e| {
            let msg = format!("load acl config file error: file={}, {}", file.display(), e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        info!("will load acl: file={}, {}", file.display(), content);

        let value: Toml = toml::from_str(&content).map_err(|e| {
            let msg = format!("invalid acl config format: file={}, {}", file.display(), e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(value)
    }
}

pub(super) struct AclLoader<'a> {
    file_loader: AclFileLoader,
    config: &'a mut AclConfig,
    acl: AclTableContainer,
}

impl<'a> AclLoader<'a> {
    pub fn new(
        file_loader: AclFileLoader,
        config: &'a mut AclConfig,
        acl: AclTableContainer,
    ) -> Self {
        Self {
            file_loader,
            config,
            acl,
        }
    }

    pub async fn load(&mut self) -> BuckyResult<()> {
        let value = self.file_loader.load_file("acl.toml")?;

        match value {
            Toml::Table(table) => {
                // 优先加载config
                // 这里弃用remove操作，因为toml的bug，依赖IndexMap的swap_remove导致remove后顺序发生变化
                if let Some(node) = table.get("config") {
                    self.config.load(node)?;
                }

                self.acl.load(table, false)?;

                Ok(())
            }
            _ => {
                let msg = format!("acl node not invalid table: {:?}", value);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
            }
        }
    }
}
