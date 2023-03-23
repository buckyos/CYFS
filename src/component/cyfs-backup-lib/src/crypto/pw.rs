use std::ops::Deref;
use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct ProtectedPassword(String);

impl ProtectedPassword {
    pub fn new(password: impl Into<String>) -> Self {
        Self(password.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Deref for ProtectedPassword {
    type Target = str;

    fn deref(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for ProtectedPassword {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[Protected Password]")
    }
}