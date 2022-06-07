use cyfs_base::*;

use toml::Value as Toml;

#[derive(Clone, Debug)]
pub struct AclConfig {
    // 一些基础的权限控制策略
    pub read_bypass_ood: bool,
    pub write_bypass_ood: bool,
}

impl Default for AclConfig {
    fn default() -> Self {
        Self {
            read_bypass_ood: true,
            write_bypass_ood: false,
        }
    }
}

impl AclConfig {
    pub fn load(&mut self, value: &Toml) -> BuckyResult<()> {
        AclConfigLoader::load(self, value)
    }
}

pub(super) struct AclConfigLoader;

impl AclConfigLoader {

    pub fn load(config: &mut AclConfig, value: &Toml) -> BuckyResult<()> {

        match value {
            Toml::Table(table) => {
                Self::load_list(config, table)
            }
            _ => {
                let msg = format!("acl [config] node not invalid table: {:?}", value);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
            }
        }
    }

    fn load_list(
        config: &mut AclConfig,
        table: &toml::value::Table,
    ) -> BuckyResult<()> {
        for (k, v) in table {
            debug!("will load acl [config] item: {:?} = {:?}", k, v);
            match k.as_str() {
                "read-bypass-ood" => {
                    match v.as_bool() {
                        Some(b) => {
                            config.read_bypass_ood = b;
                        }
                        None => {
                            let msg = format!("acl [config] node invalid type: {} = {:?}", k, v);
                            error!("{}", msg);
                            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                        }
                    }
                }
                "write-bypass-ood" => {
                    match v.as_bool() {
                        Some(b) => {
                            config.write_bypass_ood = b;
                        }
                        None => {
                            let msg = format!("acl [config] node invalid type: {} = {:?}", k, v);
                            error!("{}", msg);
                            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                        }
                    }
                }
                _ => {
                    warn!("unknown acl [config] node: {} = {:?}", k, v);
                }
            }
        }

        Ok(())
    }
}