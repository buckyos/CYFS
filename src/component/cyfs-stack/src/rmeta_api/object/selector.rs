use cyfs_base::*;
use cyfs_lib::*;

use serde::de::{Deserialize, Deserializer};
use serde::ser::{Serialize, Serializer};

#[derive(Eq, PartialEq, PartialOrd, Ord)]
pub(crate) struct ObjectSelector(ExpEvaluator);

impl std::fmt::Debug for ObjectSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.exp())?;

        Ok(())
    }
}

impl ObjectSelector {
    pub fn new(s: String) -> BuckyResult<Self> {
        let exp = ExpEvaluator::new(s, ObjectSelectorTokenList::token_list())?;

        Ok(Self(exp))
    }

    pub fn new_uninit(s: String) -> Self {
        let exp = ExpEvaluator::new_uninit(s);

        Self(exp)
    }

    pub fn exp(&self) -> &str {
        self.0.exp()
    }

    pub fn eval(&self, object_data: &dyn ObjectSelectorDataProvider) -> BuckyResult<bool> {
        self.0.eval(&object_data)
    }

    pub fn into_exp(self) -> String {
        self.0.into_exp()
    }
}

impl std::str::FromStr for ObjectSelector {
    type Err = BuckyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_owned())
    }
}

// serde codec
impl Serialize for ObjectSelector {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.exp())
    }
}

impl<'de> Deserialize<'de> for ObjectSelector {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(TStringVisitor::<Self>::new())
    }
}
