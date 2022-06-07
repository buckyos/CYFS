use std::path::PathBuf;


pub struct AccessTokenGen {
    saved_file: PathBuf,
}

impl AccessTokenGen {
    pub fn new() -> Self {
        let dir = cyfs_util::get_service_data_dir("ood-control");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            error!("create data dir error! {}, {}", dir.display(), e);
        }

        let saved_file = dir.join("access-token");

        Self { saved_file }
    }

    pub fn gen_access_token(&self, len: usize) -> String {
        match self.load_access_token() {
            Some(value) => value,
            None => {
                let token = Self::random_access_token(len);
                self.save_access_token(&token);

                token
            }
        }
    }

    fn load_access_token(&self) -> Option<String> {
        if !self.saved_file.is_file() {
            info!(
                "access-token file not exists: {}",
                self.saved_file.display()
            );
            return None;
        }

        match std::fs::read_to_string(&self.saved_file) {
            Ok(value) => {
                info!(
                    "load access-token from file: {}, {}",
                    value,
                    self.saved_file.display()
                );
                Some(value)
            }
            Err(e) => {
                error!(
                    "load access-token from file error! {}, {}",
                    self.saved_file.display(),
                    e
                );
                None
            }
        }
    }

    fn save_access_token(&self, value: &str) {
        match std::fs::write(&self.saved_file, value) {
            Ok(_) => {
                info!(
                    "save access-token to file: {}, {}",
                    value,
                    self.saved_file.display()
                );
            }
            Err(e) => {
                error!(
                    "save access-token to file error! {}, {}",
                    self.saved_file.display(),
                    e
                );
            }
        }
    }

    fn random_access_token(len: usize) -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"0123456789";

        let mut rng = rand::thread_rng();
        let token: String = (0..len)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect();
        token
    }
}