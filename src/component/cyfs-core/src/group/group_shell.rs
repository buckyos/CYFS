use cyfs_base::{Group, GroupType};

use crate::{ObjectShell, OBJECT_SHELL_ALL_FREEDOM};

pub trait GroupShell: Sized {
    fn to_shell(&self) -> ObjectShell<Group, GroupType>;
}

impl GroupShell for Group {
    fn to_shell(&self) -> ObjectShell<Group, GroupType> {
        ObjectShell::<Group, GroupType>::from_object(self.clone(), OBJECT_SHELL_ALL_FREEDOM)
    }
}
