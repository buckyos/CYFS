use cyfs_base::*;
use cyfs_lib::*;

pub(crate) struct RootStateInputHttpRequest<State> {
    pub request: tide::Request<State>,

    // 来源设备和协议
    pub source: DeviceId,
    pub protocol: NONProtocol,
}

impl<State> RootStateInputHttpRequest<State> {
    pub fn new(protocol: &NONProtocol, request: tide::Request<State>) -> Self {
        let source =
            RequestorHelper::decode_header(&request, ::cyfs_base::CYFS_REMOTE_DEVICE).unwrap();
        Self {
            request,
            source,
            protocol: protocol.to_owned(),
        }
    }
}


pub(crate) type OpEnvInputHttpRequest<State> = RootStateInputHttpRequest<State>;