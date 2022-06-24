use std::rc::{Rc};
use cyfs_base::{BuckyError, BuckyResult, BuckyErrorCode, HashValue};
use std::sync::Arc;
use base58::{FromBase58, ToBase58};
use log::error;
use cyfs_base_meta::*;

#[macro_export]
macro_rules! meta_err {
    ( $err: expr) => {
    cyfs_base::BuckyError::new(cyfs_base::BuckyErrorCodeEx::MetaError($err as u16), format!("{} {} dsg_code_err:{}", file!(), line!(), $err))
    };
}

#[macro_export]
macro_rules! meta_err2 {
    ( $err: expr, $msg: expr) => {
    cyfs_base::BuckyError::new(cyfs_base::BuckyErrorCodeEx::MetaError($err as u16), format!("{} {} msg:{}", file!(), line!(), $msg))
    };
}

#[macro_export]
macro_rules! meta_map_err {
    ( $err: expr, $old_err_code: expr, $new_err_code: expr) => {
        {
            if get_meta_err_code($err)? == $old_err_code {
                cyfs_base::BuckyError::new(cyfs_base::BuckyErrorCodeEx::MetaError($new_err_code as u16), format!("{} {} dsg_code_err:{}", file!(), line!(), $new_err_code))
            } else {
                cyfs_base::BuckyError::new($err.code(), format!("{} {} base_code_err:{}", file!(), line!(), $err))
            }
        }
    }
}

pub trait RcWeakHelper<T: ?Sized> {
    fn to_rc(&self) -> BuckyResult<Rc<T>>;
}

impl <T: ?Sized> RcWeakHelper<T> for std::rc::Weak<T> {
    fn to_rc(&self) -> BuckyResult<Rc<T>> {
        match self.upgrade() {
            Some(v) => {
                Ok(v)
            },
            None => {
                Err(meta_err!(ERROR_EXCEPTION))
            }
        }
    }
}

pub trait ArcWeakHelper<T: ?Sized> {
    fn to_rc(&self) -> BuckyResult<Arc<T>>;
}

impl <T: ?Sized> ArcWeakHelper<T> for std::sync::Weak<T> {
    fn to_rc(&self) -> BuckyResult<Arc<T>> {
        match self.upgrade() {
            Some(v) => {
                Ok(v)
            },
            None => {
                Err(meta_err!(ERROR_EXCEPTION))
            }
        }
    }
}

pub fn get_meta_err_code(ret: &BuckyError) -> BuckyResult<u16> {
    if let BuckyErrorCode::MetaError(code) = ret.code() {
        Ok(code)
    } else {
        Err(meta_err!(ERROR_EXCEPTION))
    }
}

pub trait HashValueEx {
    fn to_base58(&self) -> String;
    fn from_base58(s: &str) -> BuckyResult<HashValue>;
}

impl HashValueEx for HashValue {
    fn to_base58(&self) -> String {
        self.as_slice().to_base58()
    }

    fn from_base58(s: &str) -> BuckyResult<HashValue> {
        let buf = s.from_base58().map_err(|_e| {
            error!("convert base58 str to HashValue failed, str:{}", s);
            let msg = format!("convert base58 str to object id failed, str={}", s);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        if buf.len() != 32 {
            let msg = format!(
                "convert base58 str to object id failed, len unmatch: str={}",
                s
            );
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let mut id = Self::default();
        unsafe {
            std::ptr::copy(buf.as_ptr(), id.as_mut_slice().as_mut_ptr(), buf.len());
        }

        Ok(id)
    }
}
