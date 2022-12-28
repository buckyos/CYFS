// as Tx in blockchain

use cyfs_base::ObjectId;
use cyfs_core::GroupProposal;

pub trait AsProposal {
    fn id(&self) -> ObjectId;
    fn caller(&self) -> &ObjectId;
}

impl AsProposal for GroupProposal {
    fn id(&self) -> ObjectId {
        todo!()
    }

    fn caller(&self) -> &ObjectId {
        todo!()
    }
}
