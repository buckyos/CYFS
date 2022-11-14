use cyfs_base::*;

use std::str::FromStr;


#[derive(Debug, Eq, PartialEq)]
pub enum MetaAction {
    GlobalStateAddAccess,
    GlobalStateRemoveAccess,
    GlobalStateClearAccess,

    GlobalStateAddLink,
    GlobalStateRemoveLink,
    GlobalStateClearLink,

    GlobalStateAddObjectMeta,
    GlobalStateRemoveObjectMeta,
    GlobalStateClearObjectMeta,
}

impl ToString for MetaAction {
    fn to_string(&self) -> String {
        (match *self {
            Self::GlobalStateAddAccess => "global-state-add-access",
            Self::GlobalStateRemoveAccess => "global-state-remove-access",
            Self::GlobalStateClearAccess => "global-state-clear-access",

            Self::GlobalStateAddLink => "global-state-add-link",
            Self::GlobalStateRemoveLink => "global-state-remove-link",
            Self::GlobalStateClearLink => "global-state-clear-link",

            Self::GlobalStateAddObjectMeta => "global-state-add-object-meta",
            Self::GlobalStateRemoveObjectMeta => "global-state-remove-object-meta",
            Self::GlobalStateClearObjectMeta => "global-state-clear-object-meta",
        })
        .to_owned()
    }
}

impl FromStr for MetaAction {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "global-state-add-access" => Self::GlobalStateAddAccess,
            "global-state-remove-access" => Self::GlobalStateRemoveAccess,
            "global-state-clear-access" => Self::GlobalStateClearAccess,

            "global-state-add-link" => Self::GlobalStateAddLink,
            "global-state-remove-link" => Self::GlobalStateRemoveLink,
            "global-state-clear-link" => Self::GlobalStateClearLink,

            "global-state-add-object-meta" => Self::GlobalStateAddObjectMeta,
            "global-state-remove-object-meta" => Self::GlobalStateRemoveObjectMeta,
            "global-state-clear-object-meta" => Self::GlobalStateClearObjectMeta,

            v @ _ => {
                let msg = format!("unknown meta action: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
}