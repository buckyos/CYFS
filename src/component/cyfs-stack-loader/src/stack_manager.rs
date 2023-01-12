use crate::stack_info::StackInfo;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, DeviceId};
use cyfs_bdt::StackGuard;
use cyfs_debug::Mutex;
use cyfs_stack::CyfsStack;

use lazy_static::lazy_static;
use std::sync::Arc;

pub(crate) struct StackManagerImpl {
    stack_list: Vec<StackInfo>,
}

impl StackManagerImpl {
    pub fn new() -> Self {
        Self {
            stack_list: Vec::new(),
        }
    }

    fn load(cfg_node: toml::Value) -> BuckyResult<Vec<StackInfo>> {
        let list = Self::root_to_list(cfg_node)?;

        Self::load_list(list)
    }

    // convert single node mode to vec mode
    // 支持一级的table和两级的数组两种模式
    fn root_to_list(cfg_node: toml::Value) -> BuckyResult<Vec<toml::value::Table>> {
        match cfg_node {
            toml::Value::Table(cfg) => Ok(vec![cfg]),
            toml::Value::Array(list) => {
                let mut result = vec![];
                for cfg_node in list {
                    match cfg_node {
                        toml::Value::Table(cfg) => {
                            result.push(cfg);
                        }
                        _ => {
                            let msg = format!(
                                "stack config list item invalid format! config={:?}",
                                cfg_node
                            );
                            error!("{}", msg);
                            return Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)));
                        }
                    }
                }

                Ok(result)
            }
            _ => {
                let msg = format!(
                    "stack config root node invalid format! config={:?}",
                    cfg_node
                );
                error!("{}", msg);
                Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)))
            }
        }
    }

    fn load_list(stack_node_list: Vec<toml::value::Table>) -> BuckyResult<Vec<StackInfo>> {
        let mut list = Vec::new();
        for v in stack_node_list {
            let item = StackInfo::new();
            let item = item.load(&v).map_err(|e| {
                error!(
                    "load bdt stack error: config={}, err={}",
                    toml::to_string(&v).unwrap(),
                    e
                );
                e
            })?;

            // id不能为空
            if item.id().is_empty() {
                let msg = format!(
                    "invalid non stack config, id not found or is empty! config={}",
                    toml::to_string(&v).unwrap()
                );
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }

            info!(
                "load non stack success! id={}, config={}, device={}",
                item.id(),
                toml::to_string(&v).unwrap(),
                item.device_id()
            );

            // 初始化协议栈
            list.push(item);
        }

        Ok(list)
    }

    fn check_stack(&self, item: &StackInfo) -> BuckyResult<()> {
        // 判断是否存在，同一个id的non stack整个进程只能存在一个
        if self.exists(item.id()) {
            let msg = format!(
                "non stack with the same id already exists! id={}",
                item.id()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
        }

        // default协议栈必须被第一个加载
        if item.is_default() && self.stack_list.len() > 0 {
            let msg = format!("default non stack must be the load at first!");
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::Failed, msg));
        }

        // 存在多个协议栈情况下，确保身份不冲突
        self.check_device(&item)?;

        Ok(())
    }

    // 多个协议栈，检查身份
    fn check_device(&self, new_item: &StackInfo) -> BuckyResult<bool> {
        for item in &self.stack_list {
            // 同一个desc只能对应一个bdt stack
            if item.device_id() == new_item.device_id() {
                let msg = format!("bdt stack with the same device already exists! cur_item={}, new_item={}, desc={}", 
                    item.id(),
                    new_item.id(),
                    item.device_id(),
                    );

                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
            }
        }

        Ok(false)
    }

    pub fn exists(&self, id: &str) -> bool {
        self.stack_list.iter().any(|item| item.id() == id)
    }

    pub fn get_stack(&self, id: &str) -> Option<&StackInfo> {
        for item in &self.stack_list {
            if item.id() == id {
                return Some(item);
            }
        }

        error!("stack info not found! id={}", id);

        None
    }

    pub fn get_default_stack(&self) -> Option<&StackInfo> {
        // 遍历获取第一个名字为DEFAULT_BDT_STACK_ID的协议栈
        for item in &self.stack_list {
            if item.is_default() {
                return Some(item);
            }
        }

        // 第一个stack为默认的
        if self.stack_list.len() > 0 {
            Some(&self.stack_list[0])
        } else {
            error!("none of bdt stack not found!");
            None
        }
    }
}

// 实现迭代器，用来遍历所有的BdtStack
impl<'a> IntoIterator for &'a StackManagerImpl {
    type Item = &'a StackInfo;
    type IntoIter = std::slice::Iter<'a, StackInfo>;

    fn into_iter(self) -> Self::IntoIter {
        self.stack_list.iter()
    }
}

pub struct StackManager(Arc<Mutex<StackManagerImpl>>);

impl StackManager {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(StackManagerImpl::new())))
    }

    // 加载配置文件的stack根节点，支持列表和table两种模式，支持多个stack
    pub async fn load(&self, node: toml::Value) -> BuckyResult<()> {
        let list = StackManagerImpl::load(node)?;

        for mut item in list {
            // 避免调用栈过深，这里使用异步加载
            let this = self.0.clone();
            async_std::task::spawn(async move {
                this.lock().unwrap().check_stack(&item)?;

                if let Err(e) = item.init().await {
                    error!("init cyfs stack failed! id={}, {}", item.id(), e);
                    return Err(e);
                }

                this.lock().unwrap().stack_list.push(item);
                Ok(())
            })
            .await?;
        }

        Ok(())
    }

    pub fn exists(&self, id: &str) -> bool {
        self.0.lock().unwrap().exists(id)
    }

    // 获取指定协议栈的相关内容
    pub fn get_bdt_stack(&self, id: Option<&str>) -> Option<StackGuard> {
        let inner = self.0.lock().unwrap();
        match id {
            Some(id) => inner.get_stack(id),
            None => inner.get_default_stack(),
        }
        .map(|info| info.bdt_stack().unwrap().to_owned())
    }

    pub fn get_cyfs_stack(&self, id: Option<&str>) -> Option<CyfsStack> {
        let inner = self.0.lock().unwrap();
        match id {
            Some(id) => inner.get_stack(id),
            None => inner.get_default_stack(),
        }
        .map(|info| info.cyfs_stack().unwrap().to_owned())
    }

    pub fn get_device_id(&self, id: Option<&str>) -> Option<DeviceId> {
        let inner = self.0.lock().unwrap();
        match id {
            Some(id) => inner.get_stack(id),
            None => inner.get_default_stack(),
        }
        .map(|info| info.device_id())
    }

    // 默认协议栈，大部分情况下只会使用这些api
    pub fn get_default_bdt_stack(&self) -> Option<StackGuard> {
        self.0
            .lock()
            .unwrap()
            .get_default_stack()
            .map(|info| info.bdt_stack().unwrap().to_owned())
    }

    pub fn get_default_cyfs_stack(&self) -> Option<CyfsStack> {
        self.get_cyfs_stack(None)
    }

    pub fn get_default_device_id(&self) -> Option<DeviceId> {
        self.0
            .lock()
            .unwrap()
            .get_default_stack()
            .map(|stack| stack.device_id())
    }

    pub fn get_bdt_stack_local_addr(&self, id: Option<&str>) -> Option<String> {
        let inner = self.0.lock().unwrap();
        match id {
            Some(id) => inner.get_stack(id),
            None => inner.get_default_stack(),
        }
        .map(|info| info.local_addr())
    }
}

lazy_static! {
    pub static ref STACK_MANAGER: StackManager = StackManager::new();
}
