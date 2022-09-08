use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStatePathLinkItem {
    pub source: String,
    pub target: String,
}