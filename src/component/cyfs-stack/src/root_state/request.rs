use cyfs_base::*;
use cyfs_lib::*;
use crate::non::NONInputHttpRequest;

pub(crate) type RootStateInputHttpRequest<State> = NONInputHttpRequest<State>;
pub(crate) type OpEnvInputHttpRequest<State> = NONInputHttpRequest<State>;