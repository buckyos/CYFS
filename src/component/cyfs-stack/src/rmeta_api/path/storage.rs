use cyfs_base::*;

use std::path::PathBuf;

pub struct GlobalStatePathMetaStorage {
    file: PathBuf,
}

impl GlobalStatePathMetaStorage {
    pub fn new(isolate: &str, dec_id: &Option<ObjectId>) -> Self {
        let mut file = cyfs_util::get_cyfs_root_path().join("etc");
   
        if isolate.len() > 0 {
            file.push(isolate);
        }

        file.push("meta");
        file.push("global-state");

        let file_name = format!("{}.json", Self::get_dec_string(dec_id));
        file.push(file_name);

        Self {
            file,
        }
    }

    fn get_dec_string(dec_id: &Option<ObjectId>) -> String {
        match dec_id {
            Some(id) => {
                if id == cyfs_core::get_system_dec_app().object_id() {
                    "system".to_owned()
                } else {
                    id.to_string()
                }
            }
            None => "system".to_owned(),
        }
    }

    pub async fn save(&self, data: String) -> BuckyResult<()> {

        if !self.file.exists() {
            let dir = self.file.parent().unwrap();
            if !dir.is_dir() {
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    error!(
                        "create global-state meta dir error! dir={}, {}",
                        dir.display(),
                        e
                    );
                }
            }
        }

        async_std::fs::write(&self.file, &data).await.map_err(|e| {
            let msg = format!(
                "write global-state meta to file error! file={}, {}, {}",
                self.file.display(),
                data,
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        info!(
            "save global-state meta to file success! file={}, {}",
            self.file.display(),
            data
        );

        Ok(())
    }
}