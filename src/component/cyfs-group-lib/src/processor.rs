use std::sync::Arc;

#[async_trait::async_trait]
pub trait GroupOutputProcessor: Send + Sync {}

pub type GroupOutputProcessorRef = Arc<dyn GroupOutputProcessor>;
