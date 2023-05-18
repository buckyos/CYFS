const STATE_PATH_SEPARATOR: &str = "/";
pub const GROUP_STATE_PATH_SHELLS: &str = ".shells";
pub const GROUP_STATE_PATH_LATEST: &str = ".latest";

pub struct GroupShellStatePath {
    root: &'static str,
    shells: String,
    latest: String,
}

impl GroupShellStatePath {
    pub fn new() -> Self {
        Self {
            root: STATE_PATH_SEPARATOR,
            shells: Self::join(&["", GROUP_STATE_PATH_SHELLS]),
            latest: Self::join(&["", GROUP_STATE_PATH_SHELLS, GROUP_STATE_PATH_LATEST]),
        }
    }

    pub fn join(fields: &[&str]) -> String {
        fields.join(STATE_PATH_SEPARATOR)
    }

    pub fn root(&self) -> &str {
        self.root
    }

    pub fn shells(&self) -> &str {
        self.shells.as_str()
    }

    pub fn latest(&self) -> &str {
        self.latest.as_str()
    }

    pub fn version(&self, version: u64) -> String {
        Self::join(&[self.shells.as_str(), version.to_string().as_str()])
    }
}
