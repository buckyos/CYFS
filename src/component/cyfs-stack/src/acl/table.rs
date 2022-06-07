use super::inner::get_inner_acl;
use super::item::*;
use super::loader::AclFileLoader;
use super::relation::*;
use super::request::*;
use crate::router_handler::RouterHandlersManager;
use cyfs_base::*;
use cyfs_debug::Mutex;
use cyfs_lib::*;

use once_cell::sync::OnceCell;
use std::sync::Arc;
use toml::Value as Toml;

#[derive(Debug)]
pub enum AclItemPosition {
    Begin,
    End,
    Before(String),
    After(String),
}

#[derive(Clone)]
pub(crate) struct AclTable {
    // 自定义列表
    acl_list: Vec<Arc<AclItem>>,

    // 默认列表
    default_acl_list: Arc<OnceCell<Vec<Arc<AclItem>>>>,

    default_access: AclAccess,
}

impl AclTable {
    pub fn new(acl_list: Vec<AclItem>) -> Self {
        let acl_list: Vec<Arc<AclItem>> = acl_list.into_iter().map(|v| Arc::new(v)).collect();

        Self {
            acl_list,
            default_acl_list: Arc::new(OnceCell::new()),
            default_access: AclAccess::Reject,
        }
    }

    pub fn append_list(&mut self, acl_list: Vec<AclItem>) {
        self.acl_list
            .append(&mut acl_list.into_iter().map(|v| Arc::new(v)).collect());
    }

    // 默认acl列表，只能设置一次
    pub fn set_default_list(&mut self, acl_list: Vec<AclItem>) {
        assert!(self.default_acl_list.get().is_none());

        let list: Vec<Arc<AclItem>> = acl_list.into_iter().map(|v| Arc::new(v)).collect();
        if let Err(_) = self.default_acl_list.set(list) {
            unreachable!();
        }
    }

    pub fn add_item(&mut self, pos: AclItemPosition, list: Vec<AclItem>) -> BuckyResult<()> {
        let index;
        match pos {
            AclItemPosition::Begin => {
                index = 0;
            }
            AclItemPosition::End => {
                index = self.acl_list.len();
            }
            AclItemPosition::Before(other) => {
                index = self.item_index(&other)?;
            }
            AclItemPosition::After(other) => {
                index = self.item_index(&other)? + 1;
            }
        };

        self.acl_list
            .splice(index..index, list.into_iter().map(|v| Arc::new(v)));

        Ok(())
    }

    pub fn remove_item(&mut self, id: &str) -> BuckyResult<()> {
        let index = self.item_index(id)?;
        self.acl_list.remove(index);

        info!("remove acl item success! id={}", id);

        Ok(())
    }

    fn item_index(&self, id: &str) -> BuckyResult<usize> {
        let index = self.acl_list.iter().position(|v| v.id() == id);
        if index.is_none() {
            let msg = format!("acl item not found: {}", id);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let index = index.unwrap();
        Ok(index)
    }

    pub async fn try_match(&self, req: &dyn AclRequest) -> (Option<String>, AclAccess) {
        // 首先匹配自定义列表
        let (id, access) = Self::try_match_list(req, &self.acl_list).await;
        if access != AclAccess::Pass {
            return (id, access);
        }

        // 匹配默认列表
        let (id, access) = Self::try_match_list(req, &self.default_acl_list.get().unwrap()).await;
        if access != AclAccess::Pass {
            return (id, access);
        }

        // 都没匹配上，那么使用默认值
        info!(
            "acl match list but not found, now will use default access! access={:?}, req={}",
            self.default_access, req,
        );

        (None, self.default_access.clone())
    }

    async fn try_match_list(
        req: &dyn AclRequest,
        acl_list: &Vec<Arc<AclItem>>,
    ) -> (Option<String>, AclAccess) {

        for item in acl_list {
            match item.try_match(req).await {
                Ok(None) => {
                    continue;
                }
                Ok(Some(ret)) => match ret {
                    AclAccess::Pass => {
                        continue;
                    }
                    _ => {
                        info!(
                            "acl match item: acl={}, access={:?}, req={}",
                            item.id(),
                            ret,
                            req,
                        );
                        return (Some(item.id().to_owned()), ret);
                    }
                },
                Err(e) => {
                    info!(
                        "acl match item got error! acl={}, req={}, {}",
                        item.id(),
                        req,
                        e
                    );

                    // TODO 判断失败如何处理？认为没有命中，继续下一条acl规则
                    continue;
                }
            }
        }

        (None, AclAccess::Pass)
    }
}

pub(crate) struct AclTableLoader {
    file_loader: AclFileLoader,
    relation_manager: AclRelationManager,
    router_handlers: RouterHandlersManager,
}

const RESERVED_KEYS: [&'static str; 4] = ["action", "res", "group", "access"];

impl AclTableLoader {
    pub fn new(
        file_loader: AclFileLoader,
        router_handlers: RouterHandlersManager,
        relation_manager: AclRelationManager,
    ) -> Self {
        Self {
            file_loader,
            router_handlers,
            relation_manager,
        }
    }

    fn is_leaf_table(table: &toml::value::Table) -> bool {
        for key in RESERVED_KEYS.iter() {
            if table.contains_key(*key) {
                return true;
            }
        }

        false
    }

    pub fn load(
        &self,
        name: Option<String>,
        table: toml::value::Table,
    ) -> BuckyResult<Vec<AclItem>> {
        let mut list = vec![];

        for (k, v) in table.into_iter() {
            // 过滤config特殊节点
            if name.is_none() && k.as_str() == "config" {
                continue;
            }

            //debug!("will load acl table item: {:?} = {:?}", k, v);
            match v {
                Toml::Table(t) => {
                    let sub_name = match &name {
                        Some(name) => format!("{}.{}", name, k),
                        None => k,
                    };

                    if Self::is_leaf_table(&t) {
                        let ret = self.load_item(sub_name, t)?;
                        list.push(ret);
                    } else {
                        let mut ret = self.load(Some(sub_name), t)?;
                        list.append(&mut ret);
                    }
                }
                Toml::String(v) => {
                    if k == "include" {
                        let mut ret = self.load_include(v)?;
                        list.append(&mut ret);
                    }
                }
                _ => {
                    let msg = format!("acl config node not invalid table: {:?}", v);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                }
            }
        }

        Ok(list)
    }

    // 加载include
    fn load_include(&self, value: String) -> BuckyResult<Vec<AclItem>> {
        // 首先尝试加载内置include
        if let Some(value) = get_inner_acl(&value) {
            let table = Self::load_as_table(value).unwrap();
            return self.load(None, table);
        }

        // 尝试加载对应的文件
        let value = self.file_loader.load_file(&value)?;
        match value {
            Toml::Table(table) => self.load(None, table),
            _ => {
                let msg = format!("acl item is not invalid table: {}", value);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
            }
        }
    }

    pub fn load_as_table(value: &str) -> BuckyResult<toml::value::Table> {
        let node: Toml = toml::from_str(value).map_err(|e| {
            let msg = format!("invalid acl item format: value={}, {}", value, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        match node {
            Toml::Table(table) => Ok(table),
            _ => {
                let msg = format!("acl item is not invalid table: {}", value);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
            }
        }
    }

    fn load_item(&self, id: String, table: toml::value::Table) -> BuckyResult<AclItem> {
        debug!("will load acl item: {} = {:?}", id, table);

        AclItem::load(id, &self.router_handlers, &self.relation_manager, table)
    }
}

#[derive(Clone)]
pub(crate) struct AclTableContainer {
    table: Arc<Mutex<AclTable>>,
    file_loader: AclFileLoader,
    router_handlers: RouterHandlersManager,
    relation_manager: AclRelationManager,
}

impl AclTableContainer {
    pub(crate) fn new(
        file_loader: AclFileLoader,
        router_handlers: RouterHandlersManager,
        relation_manager: AclRelationManager,
    ) -> Self {
        Self {
            file_loader,
            table: Arc::new(Mutex::new(AclTable::new(vec![]))),
            router_handlers,
            relation_manager,
        }
    }

    pub fn load(&self, table: toml::value::Table, as_default: bool) -> BuckyResult<()> {
        let loader = AclTableLoader::new(
            self.file_loader.clone(),
            self.router_handlers.clone(),
            self.relation_manager.clone(),
        );
        let list = loader.load(None, table)?;

        info!("load acl config success!");
        {
            let mut table = self.table.lock().unwrap();

            if as_default {
                table.set_default_list(list);
            } else {
                table.append_list(list);
            }
        }

        Ok(())
    }

    pub fn add_item(&self, pos: AclItemPosition, value: &str) -> BuckyResult<()> {
        let table = AclTableLoader::load_as_table(value)?;
        let loader = AclTableLoader::new(
            self.file_loader.clone(),
            self.router_handlers.clone(),
            self.relation_manager.clone(),
        );
        let list = loader.load(None, table)?;

        info!("will add acl item: pos={:?}, value={}", pos, value);

        {
            let mut table = self.table.lock().unwrap();
            table.add_item(pos, list)?;
        }

        Ok(())
    }

    pub fn remove_item(&self, id: &str) -> BuckyResult<()> {
        {
            let mut table = self.table.lock().unwrap();
            table.remove_item(id)?;
        }

        Ok(())
    }

    pub async fn try_match(&self, req: &dyn AclRequest) -> (Option<String>, AclAccess) {
        // FIXME 这里考虑到线程安全，先拷贝一份出来处理
        let table = self.table.lock().unwrap().clone();
        table.try_match(req).await
    }
}
