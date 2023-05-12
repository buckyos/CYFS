use cyfs_base::{Group, GroupType};

use crate::{ObjectShell, OBJECT_SHELL_ALL_FREEDOM_WITH_FULL_DESC};

pub type GroupShell = ObjectShell<GroupType>;

pub trait ToGroupShell: Sized {
    fn to_shell(&self) -> GroupShell;
}

impl ToGroupShell for Group {
    fn to_shell(&self) -> GroupShell {
        GroupShell::from_object(self, OBJECT_SHELL_ALL_FREEDOM_WITH_FULL_DESC)
    }
}
