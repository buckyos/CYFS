use cyfs_base::*;

use std::str::FromStr;

#[derive(Debug, Eq, PartialEq)]
pub enum RootStateAction {
    GetCurrentRoot,
    CreateOpEnv,
}

impl ToString for RootStateAction {
    fn to_string(&self) -> String {
        (match *self {
            Self::GetCurrentRoot => "get-current-root",
            Self::CreateOpEnv => "create-op-env",
        })
        .to_owned()
    }
}

impl FromStr for RootStateAction {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "get-current-root" => Self::GetCurrentRoot,
            "create-op-env" => Self::CreateOpEnv,

            v @ _ => {
                let msg = format!("unknown state action: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum OpEnvAction {
    // map methods
    GetByKey,
    InsertWithKey,
    SetWithKey,
    RemoveWithKey,

    // set methods
    Contains,
    Insert,
    Remove,

    // single op_env
    Load,
    LoadByPath,
    CreateNew,

    // transaciton
    Lock,
    Commit,
    Abort,

    // metadata
    Metadata,

    // iterator
    Next,
}

impl ToString for OpEnvAction {
    fn to_string(&self) -> String {
        (match *self {
            Self::GetByKey => "get-by-key",
            Self::InsertWithKey => "insert-with-key",
            Self::SetWithKey => "set-with-key",
            Self::RemoveWithKey => "remove-with-key",

            Self::Contains => "contains",
            Self::Insert => "insert",
            Self::Remove => "remove",

            Self::Load => "load",
            Self::LoadByPath => "load-by-path",
            Self::CreateNew => "create-new",

            Self::Lock => "lock",
            Self::Commit => "commit",
            Self::Abort => "abort",

            Self::Metadata => "metadata",

            Self::Next => "next",
        })
        .to_owned()
    }
}

impl FromStr for OpEnvAction {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "get-by-key" => Self::GetByKey,
            "insert-with-key" => Self::InsertWithKey,
            "set-with-key" => Self::SetWithKey,
            "remove-with-key" => Self::RemoveWithKey,

            "contains" => Self::Contains,
            "insert" => Self::Insert,
            "remove" => Self::Remove,

            "load" => Self::Load,
            "load-by-path" => Self::LoadByPath,
            "create-new" => Self::CreateNew,

            "lock" => Self::Lock,
            "commit" => Self::Commit,
            "abort" => Self::Abort,

            "metadata" => Self::Metadata,
            
            "next" => Self::Next,
            
            v @ _ => {
                let msg = format!("unknown op_env action: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
}


#[derive(Debug, Eq, PartialEq)]
pub enum RootStateAccessAction {
    GetObjectByPath,
    List,
}

impl ToString for RootStateAccessAction {
    fn to_string(&self) -> String {
        (match *self {
            Self::GetObjectByPath => "get-object-by-path",
            Self::List => "list",
        })
        .to_owned()
    }
}

impl FromStr for RootStateAccessAction {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "get-object-by-path" | "get" => Self::GetObjectByPath,
            "list" => Self::List,

            _ => {
                // as default action in access mode
                Self::GetObjectByPath
            }
        };

        Ok(ret)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RootStateAccessGetMode {
    Default,
    Object,
    Data,
}

impl RootStateAccessGetMode {
    pub fn as_str(&self) -> &str {
        match *self {
            Self::Default => "default",
            Self::Object => "object",
            Self::Data => "data",
        }
    }
}

impl ToString for RootStateAccessGetMode {
    fn to_string(&self) -> String {
        self.as_str().to_owned()
    }
}

impl FromStr for RootStateAccessGetMode {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "default" => Self::Default,
            "object" => Self::Object,
            "data" => Self::Data,

            _ => {
                // as default action in access get action
                Self::Default
            }
        };

        Ok(ret)
    }
}


#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlobalStateCategory {
    RootState,
    LocalCache,
}

impl GlobalStateCategory {
    pub fn as_str(&self) -> &str {
        match &self {
            Self::RootState => "root-state",
            Self::LocalCache => "local-cache",
        }
    }
}

impl std::fmt::Display for GlobalStateCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.as_str(), f)
    }
}

impl FromStr for GlobalStateCategory {
    type Err = BuckyError;

    fn from_str(s: &str) -> BuckyResult<Self> {
        match s {
            "root-state" => Ok(Self::RootState),
            "local-cache" => Ok(Self::LocalCache),
            _ => {
                let msg = format!("unknown GlobalStateCategory value: {}", s);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::InvalidData, msg))
            }
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum GlobalStateAccessMode {
    Read = 0,
    Write = 1,
}

impl GlobalStateAccessMode {
    pub fn is_writable(&self) -> bool {
        match *self {
            GlobalStateAccessMode::Read => false,
            GlobalStateAccessMode::Write => true,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

impl ToString for GlobalStateAccessMode {
    fn to_string(&self) -> String {
        self.as_str().to_owned()
    }
}

impl FromStr for GlobalStateAccessMode {
    type Err = BuckyError;

    fn from_str(s: &str) -> BuckyResult<Self> {
        match s {
            "read" => Ok(Self::Read),
            "write" => Ok(Self::Write),
            _ => {
                let msg = format!("unknown GlobalStateAccessMode value: {}", s);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::InvalidData, msg))
            }
        }
    }
}