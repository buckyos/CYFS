mod remote_noc;
mod remote_storage;
mod storage;

pub use remote_storage::*;
pub use storage::*;

/*
use crate::*;
use cyfs_base::*;

struct StateStorage {
    dec_id: ObjectId,
    path: String,
    state_processor: GlobalStateOutputProcessorRef,
}

impl StateStorage {
    pub fn new(state_processor: GlobalStateOutputProcessorRef, path: &str) -> Self {
        let dec_id = cyfs_core::get_system_dec_app().object_id().to_owned();

        Self { dec_id, path: path.to_owned(), state_processor }
    }

    pub fn load(&self) -> BuckyResult<()> {
        let common = RootStateOutputRequestCommon {
            dec_id: Some(self.dec_id.clone()),
            target: None,
            flags: 0,
        };

        let req = RootStateCreateOpEnvOutputRequest {
            common,
            op_env_type: ObjectMapOpEnvType::Single,
        };

        let op_env = self.state_processor.create_op_env(req).await?;

        let req = OpEnvLoadByPathOutputRequest {
            common: OpEnvOutputRequestCommon {
                dec_id: Some(self.dec_id.clone()),
                target: None,
                flags: 0,
                sid: op_env.get_sid(),
            },
            path: self.path.clone(),
        };

        op_env.load_by_path(req).await
    }
}
*/