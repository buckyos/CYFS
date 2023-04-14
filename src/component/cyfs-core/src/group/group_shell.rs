use cyfs_base::{Group, GroupType};

use crate::{ObjectShell, OBJECT_SHELL_ALL_FREEDOM};

pub type GroupShell = ObjectShell<Group, GroupType>;

pub trait ToGroupShell: Sized {
    fn to_shell(&self) -> GroupShell;
}

impl ToGroupShell for Group {
    fn to_shell(&self) -> GroupShell {
        GroupShell::from_object(self.clone(), OBJECT_SHELL_ALL_FREEDOM)
    }
}
