use cyfs_base::*;

use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AclDirection {
    Any = 0,

    In = 1,
    Out = 2,
}

impl AclDirection {
    pub fn as_str(&self) -> &str {
        match *self {
            Self::Any => "*",
            Self::In => "in",
            Self::Out => "out",
        }
    }
}

impl FromStr for AclDirection {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ret = match s {
            "*" => Self::Any,
            "in" => Self::In,
            "out" => Self::Out,

            _ => {
                let msg = format!("unknown acl direction: {}", s);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }
        };

        Ok(ret)
    }
}

impl ToString for AclDirection {
    fn to_string(&self) -> String {
        self.as_str().to_owned()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AclOperationCategory {
    Both,
    Read,
    Write,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AclOperation {
    Any = 0,

    GetObject = 1,
    PutObject = 2,
    PostObject = 3,
    SelectObject = 4,
    DeleteObject = 5,

    SignObject = 6,
    VerifyObject = 7,

    PutData = 8,
    GetData = 9,
    DeleteData = 10,
    QueryFile = 11,

    // root-state
    ReadRootState = 15,
    WriteRootState = 16,

    // non/ndn的一些通用操作
    Get = 20,
    Put = 21,
    Delete = 22,

    Read = 23,
    Write = 24,

    // sign+verify
    Crypto = 25,
}

impl AclOperation {
    pub fn category(&self) -> AclOperationCategory {
        match *self {
            Self::Any => AclOperationCategory::Both,

            Self::GetObject
            | Self::SelectObject
            | Self::VerifyObject
            | Self::GetData
            | Self::ReadRootState
            | Self::Get
            | Self::Read
            | Self::QueryFile => AclOperationCategory::Read,

            Self::PutObject
            | Self::PostObject
            | Self::DeleteObject
            | Self::SignObject
            | Self::PutData
            | Self::DeleteData
            | Self::WriteRootState
            | Self::Put
            | Self::Delete
            | Self::Write
            | Self::Crypto => AclOperationCategory::Write,
        }
    }

    pub fn as_str(&self) -> &str {
        match *self {
            Self::Any => "*",
            Self::GetObject => "get-object",
            Self::PutObject => "put-object",
            Self::PostObject => "post-object",
            Self::SelectObject => "select-object",
            Self::DeleteObject => "delete-object",

            Self::SignObject => "sign-object",
            Self::VerifyObject => "verify-object",

            Self::PutData => "put-data",
            Self::GetData => "get-data",
            Self::DeleteData => "delete-data",
            Self::QueryFile => "query-file",

            Self::WriteRootState => "write-root-state",
            Self::ReadRootState => "read-root-state",

            Self::Get => "get",
            Self::Put => "put",
            Self::Delete => "delete",

            Self::Read => "read",
            Self::Write => "write",

            Self::Crypto => "crypto",
        }
    }

    pub fn is_get(&self) -> bool {
        match *self {
            Self::GetObject | Self::GetData | Self::Get | Self::Any => true,
            _ => false,
        }
    }

    pub fn is_put(&self) -> bool {
        match *self {
            Self::PutObject | Self::PutData | Self::Put | Self::Any => true,
            _ => false,
        }
    }

    pub fn is_delete(&self) -> bool {
        match *self {
            Self::DeleteObject | Self::DeleteData | Self::Delete | Self::Any => true,
            _ => false,
        }
    }

    pub fn is_crypto(&self) -> bool {
        match *self {
            Self::SignObject | Self::VerifyObject => true,
            _ => false,
        }
    }

    pub fn is_read(&self) -> bool {
        match self.category() {
            AclOperationCategory::Read | AclOperationCategory::Both => true,
            _ => false,
        }
    }

    pub fn is_write(&self) -> bool {
        match self.category() {
            AclOperationCategory::Write | AclOperationCategory::Both => true,
            _ => false,
        }
    }
}

impl FromStr for AclOperation {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ret = match s {
            "*" => Self::Any,
            "get-object" => Self::GetObject,
            "put-object" => Self::PutObject,
            "post-object" => Self::PostObject,
            "select-object" => Self::SelectObject,
            "delete-object" => Self::DeleteObject,

            "sign-object" => Self::SignObject,
            "verify-object" => Self::VerifyObject,
            "crypto" => Self::Crypto,

            "put-data" => Self::PutData,
            "get-data" => Self::GetData,
            "delete-data" => Self::DeleteData,
            "query-file" => Self::QueryFile,

            "read-root-state" => Self::ReadRootState,
            "write-root-state" => Self::WriteRootState,

            "put" => Self::Put,
            "get" => Self::Get,
            "delete" => Self::Delete,

            "read" => Self::Read,
            "write" => Self::Write,

            _ => {
                let msg = format!("unknown acl operation: {}", s);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }
        };

        Ok(ret)
    }
}

impl ToString for AclOperation {
    fn to_string(&self) -> String {
        self.as_str().to_owned()
    }
}

#[derive(Debug, Clone)]
pub struct AclAction {
    pub direction: AclDirection,
    pub operation: AclOperation,
}

impl Default for AclAction {
    fn default() -> Self {
        Self {
            direction: AclDirection::Any,
            operation: AclOperation::Any,
        }
    }
}

impl AclAction {
    pub fn new(direction: AclDirection, operation: AclOperation) -> Self {
        Self {
            direction,
            operation,
        }
    }

    pub fn is_match(&self, req: &Self) -> bool {
        assert_ne!(req.direction, AclDirection::Any);
        assert_ne!(req.operation, AclOperation::Any);

        match &self.direction {
            AclDirection::Any => {}
            _ => {
                if self.direction != req.direction {
                    return false;
                }
            }
        }

        if self.operation == req.operation {
            return true;
        }

        // 一些通用操作判断
        match &self.operation {
            AclOperation::Any => true,
            AclOperation::Get => req.operation.is_get(),
            AclOperation::Put => req.operation.is_put(),
            AclOperation::Delete => req.operation.is_delete(),
            AclOperation::Read => req.operation.is_read(),
            AclOperation::Write => req.operation.is_write(),
            AclOperation::Crypto => req.operation.is_crypto(),
            _ => false,
        }
    }

    pub fn parse(s: &str) -> BuckyResult<Self> {
        let ret = if s == "*" {
            Self {
                direction: AclDirection::Any,
                operation: AclOperation::Any,
            }
        } else {
            // 必须由至少两段组成 *-operation/direction-*/direction-operation
            let parts: Vec<&str> = s.split('-').collect();
            if parts.len() < 2 {
                let msg = format!("invalid action format: {}", s);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }

            let direction = AclDirection::from_str(parts[0])?;
            let operation = AclOperation::from_str(&parts[1..].join("-"))?;

            Self {
                direction,
                operation,
            }
        };

        Ok(ret)
    }
}
