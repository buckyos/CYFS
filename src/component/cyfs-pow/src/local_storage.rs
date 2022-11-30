use cyfs_base::*;
use crate::state::*;

use std::path::PathBuf;

pub struct PowStateLocalStorage {
    root: PathBuf,
}

impl PowStateLocalStorage {
    pub fn new_default() -> Self {
        let dir = cyfs_util::get_service_data_dir("pow");
        Self { root: dir }
    }

    fn file(&self, data: &PoWData) -> PathBuf {
        let file_name = format!("{}-{}", data.object_id, data.difficulty);
        self.root.join(&file_name)
    }
}

#[async_trait::async_trait]
impl PoWStateStorage for PowStateLocalStorage {
    async fn load(&self, data: &PoWData) -> BuckyResult<Option<PoWState>> {
        let file = self.file(data);
        if !file.exists() {
            warn!("pow local state file not found! {}", file.display());
            return Ok(None);
        }

        let s = async_std::fs::read_to_string(&file).await.map_err(|e| {
            let msg = format!(
                "read pow local state file error! file={}, {}",
                file.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let state: PoWState = serde_json::from_str(&s).map_err(|e| {
            let msg = format!(
                "parse pow local state error! file={}, value={}, {}",
                file.display(),
                s,
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        info!(
            "read pow state from local file! file={}, state={:?}",
            file.display(),
            state
        );
        Ok(Some(state))
    }

    async fn save(&self, state: &PoWState) -> BuckyResult<()> {
        let file = self.file(&state.data);
        let s = serde_json::to_string(&state).map_err(|e| {
            let msg = format!(
                "serilise pow local state error! file={}, state={:?}, {}",
                file.display(),
                state,
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        async_std::fs::write(&file, &s).await.map_err(|e| {
            let msg = format!(
                "save pow local state to file error! file={}, {}",
                file.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        info!(
            "save pow local state to file success! file={}",
            file.display()
        );
        Ok(())
    }
}