use crate::{
    bdt_loader::BdtConfigLoader, CyfsServiceLoaderConfig, KNOWN_OBJECTS_MANAGER, STACK_MANAGER,
    VAR_MANAGER,
};
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, Endpoint};
use cyfs_lib::SharedCyfsStack;
use cyfs_stack::CyfsStack;
use cyfs_util::TomlHelper;

pub struct CyfsServiceLoader;

impl CyfsServiceLoader {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn load(config: CyfsServiceLoaderConfig) -> BuckyResult<()> {
        info!(
            "non-service config: {}",
            toml::to_string(&config.node).unwrap()
        );

        Self::prepare_env().await?;

        // 加载协议栈
        if let Err(e) = STACK_MANAGER.load(config.node).await {
            error!("load stack from config failed: {}", e);
            return Err(e);
        }

        Ok(())
    }

    // 直接加载，不初始化env，外部需要保证已经调用了prepare_env
    pub async fn direct_load(config: CyfsServiceLoaderConfig) -> BuckyResult<()> {
        info!(
            "non-service config: {}",
            toml::to_string(&config.node).unwrap()
        );

        // 加载协议栈
        if let Err(e) = STACK_MANAGER.load(config.node).await {
            error!("load stack from config failed: {}", e);
            return Err(e);
        }

        Ok(())
    }

    // 准备全局环境
    pub async fn prepare_env() -> BuckyResult<()> {
        // 初始化全局变量管理器
        if let Err(e) = VAR_MANAGER.init() {
            error!("init var manager failed: {}", e);
            return Err(e);
        }

        KNOWN_OBJECTS_MANAGER.load().await;

        Ok(())
    }

    // 直接加载一组endpoints，格式如下
    /*
    [[endpoint]]
    xxxx

    [[endpoint]]
    yyyyy
    */
    pub fn load_endpoints(config: &str) -> BuckyResult<Vec<Endpoint>> {
        let node = Self::load_string_config(config)?;
        BdtConfigLoader::load_endpoints(&node)
    }

    fn load_string_config(config: &str) -> BuckyResult<Vec<toml::Value>> {
        let cfg_node: toml::Value = match toml::from_str(config) {
            Ok(v) => v,
            Err(e) => {
                let msg = format!(
                    "load endpoints toml config error, value={}, err={}",
                    config, e
                );
                error!("{}", msg);
                return Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)));
            }
        };

        let node = TomlHelper::extract_sub_node(cfg_node, "stack.bdt.endpoint")?;

        match node {
            toml::Value::Array(cfg) => {
                if cfg.is_empty() {
                    let msg = format!(
                        "stack.bdt.endpoint config node list empty! config={}",
                        config
                    );
                    error!("{}", msg);
                    Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)))
                } else {
                    Ok(cfg)
                }
            }
            _ => {
                let msg = format!(
                    "stack.bdt.endpoint config node invalid format! config={}",
                    config
                );
                error!("{}", msg);
                Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)))
            }
        }
    }

    pub fn cyfs_stack(id: Option<&str>) -> CyfsStack {
        STACK_MANAGER.get_cyfs_stack(id).unwrap()
    }

    // 必须配置了shared_stack_stub=true
    pub fn shared_cyfs_stack(id: Option<&str>) -> SharedCyfsStack {
        STACK_MANAGER.get_shared_cyfs_stack(id).unwrap()
    }

    pub fn default_object_stack() -> CyfsStack {
        STACK_MANAGER.get_default_cyfs_stack().unwrap()
    }

    // 必须配置了shared_stack_stub=true
    pub fn default_shared_object_stack() -> SharedCyfsStack {
        STACK_MANAGER.get_default_shared_cyfs_stack().unwrap()
    }
}
