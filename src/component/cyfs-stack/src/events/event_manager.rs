use super::event::*;
use cyfs_lib::*;

use once_cell::sync::OnceCell;
use std::sync::Arc;

pub struct RouterEventsContainer {
    pub test_event: OnceCell<RouterEvents<TestEventRequest, TestEventResponse>>,
}

pub type RouterEventsContainerRef = Arc<RouterEventsContainer>;

impl RouterEventsContainer {
    fn new() -> Self {
        Self {
            test_event: OnceCell::new(),
        }
    }

    pub fn test_event(&self) -> &RouterEvents<TestEventRequest, TestEventResponse> {
        self.test_event
            .get_or_init(|| RouterEvents::<TestEventRequest, TestEventResponse>::new())
    }
    pub fn try_test_event(&self) -> Option<&RouterEvents<TestEventRequest, TestEventResponse>> {
        self.test_event.get()
    }
}

#[derive(Clone)]
pub struct RouterEventsManager {
    all: Arc<RouterEventsContainer>,
}

impl RouterEventsManager {
    pub fn new() -> Self {
        let ret = Self {
            all: Arc::new(RouterEventsContainer::new()),
        };

        ret
    }

    pub fn clone_processor(&self) -> RouterEventManagerProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    pub fn events(&self) -> &Arc<RouterEventsContainer> {
        &self.all
    }
}
