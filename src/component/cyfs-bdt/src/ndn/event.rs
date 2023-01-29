use cyfs_base::*;
use crate::{
    stack::{Stack}, 
};
use super::{
    channel::{*, protocol::v0::*}, 
};


#[async_trait::async_trait]
pub trait NdnEventHandler: Send + Sync {
    async fn on_newly_interest(
        &self, 
        stack: &Stack, 
        interest: &Interest, 
        from: &Channel
    ) -> BuckyResult<()>;

    fn on_unknown_piece_data(
        &self, 
        stack: &Stack, 
        piece: &PieceData, 
        from: &Channel
    ) -> BuckyResult<DownloadSession>;
}
