use crate::*;

use std::borrow::Cow;

pub struct OpEnvPathAccess {
    limit_path: String,
    access: AccessPermissions,
}

impl OpEnvPathAccess {
    pub fn new(path: &str, access: AccessPermissions) -> Self {
        let limit_path = Self::fix_path(path).into_owned();

        Self { limit_path, access }
    }

    // 以/开头，并以/结尾
    fn fix_path(path: &str) -> Cow<str> {
        if path.starts_with('/') {
            if path.ends_with('/') {
                Cow::Borrowed(path)
            } else {
                Cow::Owned(format!("{}/", path))
            }
        } else {
            if path.ends_with('/') {
                Cow::Owned(format!("/{}", path))
            } else {
                Cow::Owned(format!("/{}/", path))
            }
        }
    }

    pub fn check_full_path(&self, full_path: &str, op_type: RequestOpType) -> BuckyResult<()> {
        let full_path = Self::fix_path(full_path);
        assert!(full_path.starts_with('/'));

        if full_path.starts_with(self.limit_path.as_str()) {
            if self.access.test_op(op_type) {
                Ok(())
            } else {
                let msg = format!("op is not allowed within path limiter! path={}, limiter={}, access={}, op='{:?}'", 
                            full_path, self.limit_path, self.access.as_str(), op_type);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
            }
        } else {
            let msg = format!(
                "full path is out of path limiter! path={}, limiter={}",
                full_path, self.limit_path
            );
            error!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
        }
    }

    pub fn check_full_path_list(
        &self,
        list: &Vec<String>,
        op_type: RequestOpType,
    ) -> BuckyResult<()> {

        for full_path in list {
            self.check_full_path(full_path.as_str(), op_type)?;
        }

        Ok(())
    }

    pub fn check_path_key(&self, path: &str, key: &str, op_type: RequestOpType) -> BuckyResult<()> {
        // assert!(path.starts_with('/'));

        let full_path = if path.ends_with('/') {
            format!("{}{}", path, key)
        } else {
            format!("{}/{}", path, key)
        };

        self.check_full_path(&full_path, op_type)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let limiter = OpEnvPathAccess::new("/a/b/c", AccessPermissions::ReadAndCall);
        limiter
            .check_full_path("/a/b", RequestOpType::Read)
            .unwrap_err();
        limiter
            .check_full_path("/a/d", RequestOpType::Call)
            .unwrap_err();
        limiter
            .check_full_path("/a/d/c1", RequestOpType::Call)
            .unwrap_err();
        limiter
            .check_full_path("/", RequestOpType::Call)
            .unwrap_err();
        limiter
            .check_full_path("/a", RequestOpType::Call)
            .unwrap_err();

        limiter
            .check_full_path("/a/b/c", RequestOpType::Call)
            .unwrap();
        limiter
            .check_full_path("/a/b/c/x", RequestOpType::Read)
            .unwrap();

        limiter
            .check_full_path("/a/b/c", RequestOpType::Write)
            .unwrap_err();
        limiter
            .check_full_path("/a/b/c/x", RequestOpType::Write)
            .unwrap_err();
    }
}
