use crate::non::*;
use cyfs_base::*;

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct CryptoOutputRequestCommon {
    // 请求路径，可为空
    pub req_path: Option<String>,

    // 来源DEC
    pub dec_id: Option<ObjectId>,

    // 用以默认行为
    pub target: Option<ObjectId>,

    pub flags: u32,
}

impl Default for CryptoOutputRequestCommon {
    fn default() -> Self {
        Self {
            req_path: None,
            dec_id: None,
            target: None,
            flags: 0,
        }
    }
}

impl fmt::Display for CryptoOutputRequestCommon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "req_path: {:?}", self.req_path)?;

        if let Some(dec_id) = &self.dec_id {
            write!(f, ", dec_id: {}", dec_id)?;
        }

        if let Some(target) = &self.target {
            write!(f, ", target: {}", target.to_string())?;
        }

        write!(f, ", flags: {}", self.flags)?;

        Ok(())
    }
}

//// sign
///
// 可以选择使用people签名还是device签名
pub const CRYPTO_REQUEST_FLAG_SIGN_BY_PEOPLE: u32 = 0x01 << 1;
pub const CRYPTO_REQUEST_FLAG_SIGN_BY_DEVICE: u32 = 0x01 << 2;

// (desc, body) * (set, push)，优先使用set > push
pub const CRYPTO_REQUEST_FLAG_SIGN_SET_DESC: u32 = 0x01 << 3;
pub const CRYPTO_REQUEST_FLAG_SIGN_SET_BODY: u32 = 0x01 << 4;
pub const CRYPTO_REQUEST_FLAG_SIGN_PUSH_DESC: u32 = 0x01 << 5;
pub const CRYPTO_REQUEST_FLAG_SIGN_PUSH_BODY: u32 = 0x01 << 6;

pub struct CryptoSignObjectOutputRequest {
    pub common: CryptoOutputRequestCommon,

    pub object: NONObjectInfo,

    pub flags: u32,
}

impl CryptoSignObjectOutputRequest {
    pub fn new(object_id: ObjectId, object_raw: Vec<u8>, flags: u32) -> Self {
        Self {
            common: CryptoOutputRequestCommon::default(),
            object: NONObjectInfo::new(object_id, object_raw, None),
            flags,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SignObjectResult {
    Signed,
    Pending,
}

impl ToString for SignObjectResult {
    fn to_string(&self) -> String {
        (match *self {
            Self::Signed => "signed",
            Self::Pending => "pending",
        })
        .to_owned()
    }
}

impl FromStr for SignObjectResult {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "signed" => Self::Signed,
            "pending" => Self::Pending,
            v @ _ => {
                let msg = format!("unknown sign object result : {}", v);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        Ok(ret)
    }
}

#[derive(Debug, Clone)]
pub struct CryptoSignObjectOutputResponse {
    pub result: SignObjectResult,

    pub object: Option<NONObjectInfo>,
}

impl fmt::Display for CryptoSignObjectOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "result: {:?}", self.result)?;

        if let Some(object) = &self.object {
            write!(f, ", object: {:?}", object)?;
        }
        Ok(())
    }
}

//// verify
///

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum VerifySignType {
    Desc,
    Body,
    Both,
}

impl VerifySignType {
    pub fn desc(&self) -> bool {
        match *self {
            Self::Desc | Self::Both => true,
            _ => false,
        }
    }

    pub fn body(&self) -> bool {
        match *self {
            Self::Body | Self::Both => true,
            _ => false,
        }
    }
}
impl ToString for VerifySignType {
    fn to_string(&self) -> String {
        (match *self {
            Self::Desc => "desc",
            Self::Body => "body",
            Self::Both => "both",
        })
        .to_owned()
    }
}

impl FromStr for VerifySignType {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "desc" => Self::Desc,
            "body" => Self::Body,
            "both" => Self::Both,
            v @ _ => {
                let msg = format!("unknown verify sign type : {}", v);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        Ok(ret)
    }
}


// 需要校验的签名列表
#[derive(Debug, Clone)]
pub struct VerifySigns {
    pub desc_signs: Option<Vec<u8>>,
    pub body_signs: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub enum VerifyObjectType {
    // 校验是否有owner的有效签名
    Owner,

    // 自校验
    Own,

    // 校验是否有指定object的有效签名
    Object(NONSlimObjectInfo),

    // 校验指定的签名是否有效
    Sign(VerifySigns),
}

impl VerifyObjectType {
    pub fn as_str(&self) -> &str {
        match *self {
            Self::Owner => "owner",
            Self::Own => "own",
            Self::Object(_) => "object",
            Self::Sign(_) => "sign",
        }
    }
}

impl ToString for VerifyObjectType {
    fn to_string(&self) -> String {
       self.as_str().to_owned()
    }
}

#[derive(Debug, Clone)]
pub struct CryptoVerifyObjectOutputRequest {
    pub common: CryptoOutputRequestCommon,

    // 校验的签名位置
    pub sign_type: VerifySignType,

    // 被校验对象
    pub object: NONObjectInfo,

    // 签名来源对象
    pub sign_object: VerifyObjectType,
}

impl CryptoVerifyObjectOutputRequest {
    pub fn new_verify_by_owner(sign_type: VerifySignType, object: NONObjectInfo) -> Self {
        Self {
            common: CryptoOutputRequestCommon::default(),
            sign_type,
            object,
            sign_object: VerifyObjectType::Owner,
        }
    }

    pub fn new_verify_by_own(object: NONObjectInfo) -> Self {
        Self {
            common: CryptoOutputRequestCommon::default(),
            // 自校验只需要校验body即可
            sign_type: VerifySignType::Body,    
            object,
            sign_object: VerifyObjectType::Owner,
        }
    }

    pub fn new_verify_by_object(
        sign_type: VerifySignType,
        object: NONObjectInfo,
        sign_object: NONSlimObjectInfo,
    ) -> Self {
        Self {
            common: CryptoOutputRequestCommon::default(),
            sign_type,
            object,
            sign_object: VerifyObjectType::Object(sign_object),
        }
    }

    pub fn new_verify_by_signs(
        sign_type: VerifySignType,
        object: NONObjectInfo,
        signs: VerifySigns,
    ) -> Self {
        Self {
            common: CryptoOutputRequestCommon::default(),
            sign_type,
            object,
            sign_object: VerifyObjectType::Sign(signs),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VerifySignResult {
    pub index: u8,
    pub valid: bool,
    pub sign_object_id: ObjectId,
}

#[derive(Debug, Clone)]
pub struct VerifyObjectResult {
    pub valid: bool,

    pub desc_signs: Vec<VerifySignResult>,
    pub body_signs: Vec<VerifySignResult>,
}

impl Default for VerifyObjectResult {
    fn default() -> Self {
        Self {
            valid: false,
            desc_signs: Vec::new(),
            body_signs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CryptoVerifyObjectOutputResponse {
    pub result: VerifyObjectResult,
}

impl fmt::Display for CryptoVerifyObjectOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "result: {:?}", self.result)
    }
}
