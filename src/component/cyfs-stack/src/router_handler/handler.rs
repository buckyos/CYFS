use super::storage::RouterHandlerSavedData;
use super::storage::RouterHandlersStorage;
use cyfs_base::*;
use cyfs_debug::Mutex;
use cyfs_lib::*;
use cyfs_util::*;

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

pub struct RouterHandler<REQ, RESP>
where
    REQ: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<RESP> + fmt::Display,
    RouterHandlerRequest<REQ, RESP>: RouterHandlerCategoryInfo,
{
    pub index: i32,

    pub id: String,

    pub dec_id: Option<ObjectId>,

    pub filter: ExpEvaluator,

    pub default_action: RouterHandlerAction,

    pub routine: Option<
        Box<
            dyn EventListenerAsyncRoutine<
                RouterHandlerRequest<REQ, RESP>,
                RouterHandlerResponse<REQ, RESP>,
            >,
        >,
    >,
}

impl<REQ, RESP> RouterHandler<REQ, RESP>
where
    REQ: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<RESP> + fmt::Display,
    RouterHandlerRequest<REQ, RESP>: RouterHandlerCategoryInfo,
{
    pub fn compare_id(&self, other: &Self) -> bool {
        self.id == other.id && self.dec_id == other.dec_id
    }

    pub fn eq(&self, other: &Self) -> bool {
        self.index == other.index
            && self.id == other.id
            && self.dec_id == other.dec_id
            && self.filter.exp() == other.filter.exp()
            && self.default_action == other.default_action
    }

    pub fn new(
        id: impl Into<String>,
        dec_id: Option<ObjectId>,
        index: i32,
        filter: &str,
        default_action: RouterHandlerAction,
        routine: Option<
            Box<
                dyn EventListenerAsyncRoutine<
                    RouterHandlerRequest<REQ, RESP>,
                    RouterHandlerResponse<REQ, RESP>,
                >,
            >,
        >,
    ) -> BuckyResult<Self> {
        // ??????filter?????????
        let reserved_token_list = ROUTER_HANDLER_RESERVED_TOKEN_LIST.select::<REQ, RESP>();
        let filter = ExpEvaluator::new(filter, reserved_token_list)?;

        let handler = RouterHandler::<REQ, RESP> {
            id: id.into(),
            dec_id,
            index,
            filter,
            default_action,
            routine,
        };

        Ok(handler)
    }
}

pub(crate) struct RouterHandlersImpl<REQ, RESP>
where
    REQ: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<RESP> + fmt::Display,
    RouterHandlerRequest<REQ, RESP>: RouterHandlerCategoryInfo,
{
    chain: RouterHandlerChain,
    handler_list: Vec<Arc<RouterHandler<REQ, RESP>>>,
}

impl<REQ, RESP> RouterHandlersImpl<REQ, RESP>
where
    REQ: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<RESP> + fmt::Display,
    RouterHandlerRequest<REQ, RESP>: RouterHandlerCategoryInfo,
{
    pub fn new(chain: RouterHandlerChain) -> Self {
        Self {
            chain,
            handler_list: Vec::new(),
        }
    }

    pub fn listener_count(&self) -> usize {
        self.handler_list.len()
    }

    pub fn category() -> RouterHandlerCategory {
        RouterHandlerRequest::<REQ, RESP>::category()
    }

    pub fn add_handler(&mut self, handler: RouterHandler<REQ, RESP>) -> BuckyResult<bool> {
        let handler = Arc::new(handler);

        let changed = (|| {
            for i in 0..self.handler_list.len() {
                let cur = &self.handler_list[i];
                if cur.compare_id(&handler) {
                    // ??????????????????
                    let changed;
                    if cur.eq(&handler) {
                        info!(
                            "router handler already exists! chain={}, category={}, id={}, dec={:?}",
                            self.chain,
                            Self::category(),
                            handler.id,
                            handler.dec_id,
                        );
                        changed = false;
                    } else {
                        info!(
                            "will replace router handler: chain={}, category={}, id={}, dec={:?}, index={}, filter={}, default_action={}, routine={}",
                            self.chain,
                            Self::category(),
                            handler.id,
                            handler.dec_id,
                            handler.index,
                            handler.filter.exp(),
                            handler.default_action,
                            handler.routine.is_some(),
                        );
                        changed = true;
                    }
                    // ?????????????????????????????????????????????routine???????????????
                    self.handler_list[i] = handler;
                    return changed;
                }
            }

            info!(
                "new router handler: chain={}, category={}, id={}, dec={:?}, index={}, filter={}, default_action={}, routine={}",
                self.chain,
                Self::category(),
                handler.id,
                handler.dec_id,
                handler.index,
                handler.filter.exp(),
                handler.default_action,
                handler.routine.is_some(),
            );
            self.handler_list.push(handler);

            true
        })();

        if changed {
            // ??????index??????????????????????????????
            self.handler_list
                .sort_by(|a, b| a.index.partial_cmp(&b.index).unwrap());
        }

        Ok(changed)
    }

    pub fn remove_handler(&mut self, id: &str, dec_id: Option<ObjectId>) -> bool {
        for i in 0..self.handler_list.len() {
            let item = &self.handler_list[i];
            if item.id == id && item.dec_id == dec_id {
                info!(
                    "will remove router handler: chain={}, category={}, id={}, dec={:?}",
                    self.chain,
                    Self::category(),
                    id,
                    dec_id,
                );
                self.handler_list.remove(i);
                return true;
            }
        }

        warn!(
            "router handler not found! chain={}, category={}, id={}, dec={:?}",
            self.chain,
            Self::category(),
            id,
            dec_id,
        );

        false
    }

    pub fn get_handler(&self, param: &REQ) -> Option<Arc<RouterHandler<REQ, RESP>>> {
        for handler in &self.handler_list {
            trace!(
                "will eval: chain={}, category={}, exp={}",
                self.chain,
                Self::category(),
                handler.filter
            );
            if handler.filter.eval(param).unwrap() {
                debug!(
                    "router handler select filter: chain={}, category={}, param={}, handler={}",
                    self.chain,
                    Self::category(),
                    param,
                    handler.id
                );
                return Some(handler.clone());
            }
        }

        None
    }

    pub fn clear_dec_handlers(&mut self, dec_id: &Option<ObjectId>) -> bool {
        let mut changed = false;
        let mut i = 0;
        while i < self.handler_list.len() {
            if self.handler_list[i].dec_id == *dec_id {
                self.handler_list.remove(i);
                changed = true;
            } else {
                i += 1;
            }
        }

        changed
        // self.handler_list.drain_filter(|item| item.dec_id.as_ref() == dec_id)
    }

    // handlers????????????????????????filter???
    pub async fn emit(
        chain: &RouterHandlerChain,
        category: &RouterHandlerCategory,
        handler: Arc<RouterHandler<REQ, RESP>>,
        param: &RouterHandlerRequest<REQ, RESP>,
    ) -> RouterHandlerResponse<REQ, RESP> {
        let resp = if handler.routine.is_some() {
            info!(
                "will emit handler routine: chain={}, category={}, id={}, dec={:?}, param={}",
                chain, category, handler.id, handler.dec_id, param
            );

            match handler.routine.as_ref().unwrap().call(&param).await {
                Ok(resp) => {
                    info!(
                        "emit handler routine success: chain={}, category={}, id={}, dec={:?}, action={}",
                        chain, category, handler.id, handler.dec_id, resp.action
                    );
                    resp
                }
                Err(e) => {
                    error!(
                        "emit handler routine error, will use default action: chain={}, category={}, id={}, dec={:?}, default action={}, {}",
                        chain, category, handler.id, handler.dec_id, handler.default_action, e
                    );

                    // ????????????????????????????????????action
                    RouterHandlerResponse {
                        action: handler.default_action.clone(),
                        request: None,
                        response: None,
                    }
                }
            }
        } else {
            RouterHandlerResponse {
                action: handler.default_action.clone(),
                request: None,
                response: None,
            }
        };

        resp
    }

    pub(crate) fn dump_data(&self) -> Option<BTreeMap<String, RouterHandlerSavedData>> {
        if self.handler_list.is_empty() {
            return None;
        }

        let mut list = BTreeMap::new();
        for item in &self.handler_list {
            // only save handler item with dec_id is not empty
            if item.dec_id.is_none() {
                continue;
            }

            let data = RouterHandlerSavedData {
                index: item.index,
                dec_id: item.dec_id.clone(),
                filter: item.filter.exp().to_owned(),
                default_action: item.default_action.to_string(),
            };

            list.insert(item.id.clone(), data);
        }
        Some(list)
    }

    pub(crate) fn load_data(&mut self, list: BTreeMap<String, RouterHandlerSavedData>) {
        for (id, item) in list.into_iter() {
            if let Err(e) = self.add_handler_from_saved_data(id, item) {
                error!(
                    "add handler from saved data error! chain={}, category={}, {}",
                    self.chain,
                    Self::category(),
                    e
                );
            }
        }
    }

    fn add_handler_from_saved_data(
        &mut self,
        id: String,
        data: RouterHandlerSavedData,
    ) -> BuckyResult<bool> {
        // will ignore all system's router handlers!
        if data.dec_id.is_none() {
            return Ok(false);
        }

        let reserved_token_list = ROUTER_HANDLER_RESERVED_TOKEN_LIST.select::<REQ, RESP>();
        let filter = ExpEvaluator::new(&data.filter, reserved_token_list)?;

        info!(
            "new handler from saved data: chain={}, category={}, {:?}",
            self.chain,
            Self::category(),
            data
        );

        let handler = RouterHandler::<REQ, RESP> {
            id,
            dec_id: data.dec_id,
            index: data.index,
            filter,
            default_action: RouterHandlerAction::from_str(&data.default_action)?,
            routine: None,
        };

        self.add_handler(handler)
    }
}

pub struct RouterHandlers<REQ, RESP>
where
    REQ: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<RESP> + fmt::Display,
    RouterHandlerRequest<REQ, RESP>: ExpReservedTokenTranslator + RouterHandlerCategoryInfo,
{
    storage: RouterHandlersStorage,
    handlers: Arc<Mutex<RouterHandlersImpl<REQ, RESP>>>,
}

impl<REQ, RESP> RouterHandlers<REQ, RESP>
where
    REQ: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<RESP> + fmt::Display,
    RouterHandlerRequest<REQ, RESP>: ExpReservedTokenTranslator + RouterHandlerCategoryInfo,
{
    pub fn new(chain: RouterHandlerChain, storage: RouterHandlersStorage) -> Self {
        Self {
            storage,
            handlers: Arc::new(Mutex::new(RouterHandlersImpl::<REQ, RESP>::new(chain))),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.listener_count() == 0
    }

    pub fn listener_count(&self) -> usize {
        let inner = self.handlers.lock().unwrap();
        inner.listener_count()
    }


    pub fn add_handler(&self, handler: RouterHandler<REQ, RESP>) -> BuckyResult<()> {

        if let Some(dec_id) = &handler.dec_id {
            self.storage.on_dec_register(dec_id);
        }

        let mut inner = self.handlers.lock().unwrap();
        let changed = inner.add_handler(handler)?;
        if changed {
            self.storage.async_save();
        }
        Ok(())
    }

    pub fn remove_handler(&self, id: &str, dec_id: Option<ObjectId>) -> bool {
        let mut inner = self.handlers.lock().unwrap();
        let ret = inner.remove_handler(id, dec_id);
        if ret {
            self.storage.async_save();
        }

        ret
    }

    pub fn clear_dec_handlers(&self, dec_id:& Option<ObjectId>) -> bool {
        let mut inner = self.handlers.lock().unwrap();
        inner.clear_dec_handlers(dec_id)
    }

    pub(crate) fn emitter(&self) -> RouterHandlerEmitter<REQ, RESP> {
        RouterHandlerEmitter::<REQ, RESP>::new(self)
    }

    pub(crate) fn specified_emitter(&self, id: &str) -> Option<RouterHandlerEmitter<REQ, RESP>> {
        RouterHandlerEmitter::<REQ, RESP>::new_with_specified(self, id)
    }

    pub(crate) fn dump_data(&self) -> Option<BTreeMap<String, RouterHandlerSavedData>> {
        let inner = self.handlers.lock().unwrap();
        inner.dump_data()
    }

    pub(crate) fn load_data(&self, list: BTreeMap<String, RouterHandlerSavedData>) {
        let mut inner = self.handlers.lock().unwrap();
        inner.load_data(list);
    }
}

pub(crate) struct RouterHandlerEmitter<REQ, RESP>
where
    REQ: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<RESP> + fmt::Display,
    RouterHandlerRequest<REQ, RESP>: ExpReservedTokenTranslator + RouterHandlerCategoryInfo,
{
    chain: RouterHandlerChain,
    category: RouterHandlerCategory,
    handler_list: Vec<Arc<RouterHandler<REQ, RESP>>>,
    next_index: usize,
}

impl<REQ, RESP> RouterHandlerEmitter<REQ, RESP>
where
    REQ: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<RESP> + fmt::Display,
    RouterHandlerRequest<REQ, RESP>: ExpReservedTokenTranslator + RouterHandlerCategoryInfo,
{
    pub fn chain(&self) -> &RouterHandlerChain {
        &self.chain
    }

    pub fn category(&self) -> &RouterHandlerCategory {
        &self.category
    }

    fn new(handlers: &RouterHandlers<REQ, RESP>) -> Self {
        let handlers = handlers.handlers.lock().unwrap();
        Self {
            chain: handlers.chain.clone(),
            category: RouterHandlersImpl::<REQ, RESP>::category(),
            handler_list: handlers.handler_list.clone(),
            next_index: 0,
        }
    }

    fn new_with_specified(handlers: &RouterHandlers<REQ, RESP>, id: &str) -> Option<Self> {
        let handlers = handlers.handlers.lock().unwrap();
        for handler in &handlers.handler_list {
            if handler.id == id {
                return Some(Self {
                    chain: handlers.chain.clone(),
                    category: RouterHandlersImpl::<REQ, RESP>::category(),
                    handler_list: vec![handler.clone()],
                    next_index: 0,
                });
            }
        }

        None
    }

    fn next_handler(
        &mut self,
        param: &RouterHandlerRequest<REQ, RESP>,
    ) -> Option<Arc<RouterHandler<REQ, RESP>>> {
        while self.next_index < self.handler_list.len() {
            let handler = &self.handler_list[self.next_index];
            self.next_index += 1;

            trace!(
                "will eval: chain={}, category={}, id={}, exp={}",
                self.chain,
                self.category,
                handler.id,
                handler.filter
            );

            if handler.filter.eval(param).unwrap() {
                debug!(
                    "router handler select filter: chain={}, category={}, param={}, handler={}",
                    self.chain, self.category, param, handler.id
                );
                return Some(handler.clone());
            }
        }

        None
    }

    pub async fn next(
        &mut self,
        param: &RouterHandlerRequest<REQ, RESP>,
        default_action: &RouterHandlerAction,
    ) -> RouterHandlerResponse<REQ, RESP> {
        // assert_ne!(*default_action, RouterHandlerAction::Pass);

        match self.next_handler(&param) {
            Some(handler) => {
                RouterHandlersImpl::emit(&self.chain, &self.category, handler, &param).await
            }
            None => RouterHandlerResponse {
                action: default_action.to_owned(),
                request: None,
                response: None,
            },
        }
    }
}
