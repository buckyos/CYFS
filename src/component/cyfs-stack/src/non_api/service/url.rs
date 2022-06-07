use cyfs_base::*;

use std::str::FromStr;

// object_id base58编码后的长度范围
const PATH_SEGMENT_OBJECT_ID_MIN_LEN: usize = 42;
const PATH_SEGMENT_OBJECT_ID_MAX_LEN: usize = 45;

pub(crate) struct NONGetObjectUrlParam {
    pub req_path: Option<String>,

    pub object_id: ObjectId,
    pub inner_path: Option<String>,
}

pub(crate) struct NONPutObjectUrlParam {
    pub req_path: Option<String>,

    pub object_id: ObjectId,
}

pub(crate) struct NONSelectObjectUrlParam {
    pub req_path: Option<String>,
}

// 解析 [/req_path][/object_id]类型的url
pub(crate) struct NONOptionObjectUrlParam {
    pub req_path: Option<String>,

    pub object_id: Option<ObjectId>,
}

pub(crate) struct NONRequestUrlParser {}

impl NONRequestUrlParser {
    fn parse_seg_object_id(seg: &str) -> Option<ObjectId> {
        // 只对合适的字符串才尝试解析是不是object_id
        if seg.len() >= PATH_SEGMENT_OBJECT_ID_MIN_LEN
            && seg.len() <= PATH_SEGMENT_OBJECT_ID_MAX_LEN
        {
            match ObjectId::from_str(seg) {
                Ok(id) => {
                    return Some(id);
                }
                Err(_) => {
                    // 作为path
                }
            };
        }

        None
    }

    fn extract_param<State>(req: &tide::Request<State>) -> BuckyResult<Option<String>> {
        match req.param("must") {
            Ok(v) => {
                // 对url里面的以%编码的unicode字符进行解码
                let decoded_value = percent_encoding::percent_decode_str(&v);
                let value = decoded_value.decode_utf8().map_err(|e| {
                    let msg = format!("invalid utf8 url format! param={}, {}", v, e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                })?;

                Ok(Some(value.into_owned()))
            }
            Err(_) => {
                // 找不到的情况下，req.param会返回错误
                Ok(None)
            }
        }
    }

    fn extract_no_empty_param<State>(req: &tide::Request<State>) -> BuckyResult<String> {
        match Self::extract_param(req)? {
            Some(v) => Ok(v),
            None => {
                let msg = format!("request url param missing! {}", req.url());
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg))
            }
        }
    }

    fn parse_param(param: &str) -> Vec<&str> {
        let parts: Vec<&str> = param.split('/').filter(|seg| !seg.is_empty()).collect();
        parts
    }

    // [/req_path]/object_id
    pub fn parse_put_param<State>(req: &tide::Request<State>) -> BuckyResult<NONPutObjectUrlParam> {
        let left = Self::extract_no_empty_param(req)?;
        let mut parts = Self::parse_param(&left);

        if parts.is_empty() {
            let msg = format!(
                "invalid non put_object url param, object_id not found! {}",
                left
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let object_id = Self::parse_seg_object_id(parts.pop().unwrap()).ok_or_else(|| {
            let msg = format!(
                "invalid non put_object url param, last seg should be valid object_id! {}",
                left
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        // 剩余的部分，作为req_path
        let req_path = if parts.len() > 0 {
            Some("/".to_owned() + &parts.join("/"))
        } else {
            None
        };

        let ret = NONPutObjectUrlParam {
            req_path,
            object_id,
        };

        Ok(ret)
    }

    // [/req_path][/object_id]
    pub fn parse_option_object_param<State>(
        req: &tide::Request<State>,
    ) -> BuckyResult<NONOptionObjectUrlParam> {
        let param = Self::extract_param(req)?;
        let mut parts = match &param {
            Some(left) => Self::parse_param(&left),
            None => vec![],
        };

        let mut ret = NONOptionObjectUrlParam {
            req_path: None,
            object_id: None,
        };

        if parts.is_empty() {
            return Ok(ret);
        }

        ret.object_id = Self::parse_seg_object_id(parts.last().unwrap());
        if ret.object_id.is_some() {
            parts.pop();
        }

        // 剩余的部分，作为req_path
        ret.req_path = if parts.len() > 0 {
            Some("/".to_owned() + &parts.join("/"))
        } else {
            None
        };

        Ok(ret)
    }

    // [/req_path]
    pub fn parse_select_param<State>(
        req: &tide::Request<State>,
    ) -> BuckyResult<NONSelectObjectUrlParam> {
        let left = Self::extract_param(req)?;
        let req_path = match left {
            Some(left) => {
                let v = left.trim();
                if v.len() > 0 {
                    Some(v.to_owned())
                } else {
                    None
                }
            }
            None => None,
        };

        Ok(NONSelectObjectUrlParam { req_path })
    }

    /*
    [/req_path]/object_id
    [/req_path]/dir_id/inner_path
    */
    pub fn parse_get_param<State>(req: &tide::Request<State>) -> BuckyResult<NONGetObjectUrlParam> {
        let left = Self::extract_no_empty_param(req)?;
        let mut parts = Self::parse_param(&left);

        // 从后向前面找，找到第一个object_id
        let mut inner_path = vec![];
        let object_id;

        loop {
            let part = parts.pop();
            if part.is_none() {
                let msg = format!(
                    "invalid non get_object url param, object_id missing or invalid! {}",
                    left
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }

            let part = part.unwrap();
            match Self::parse_seg_object_id(part) {
                Some(id) => {
                    object_id = id;
                    break;
                }
                None => {
                    inner_path.push(part);
                }
            }
        }

        // 对于dir，支持inner_path
        let inner_path = if inner_path.len() > 0 {
            inner_path.reverse();
            Some("/".to_owned() + &inner_path.join("/"))
        } else {
            None
        };

        // 剩余的部分，作为req_path
        let req_path = if parts.len() > 0 {
            Some("/".to_owned() + &parts.join("/"))
        } else {
            None
        };

        let ret = NONGetObjectUrlParam {
            req_path,
            object_id,
            inner_path,
        };

        Ok(ret)
    }
}
