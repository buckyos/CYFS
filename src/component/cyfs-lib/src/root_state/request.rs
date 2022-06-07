use super::output_request::*;

pub type RootStateRequestCommon = RootStateOutputRequestCommon;

pub type RootStateGetCurrentRootRequest = RootStateGetCurrentRootOutputRequest;
pub type RootStateGetCurrentRootResponse = RootStateGetCurrentRootOutputResponse;

pub type RootStateCreateOpEnvRequest = RootStateCreateOpEnvOutputRequest;
pub type RootStateCreateOpEnvResponse = RootStateCreateOpEnvOutputResponse;

pub type OpEnvRequestCommon = OpEnvOutputRequestCommon;

pub type OpEnvLoadRequest = OpEnvLoadOutputRequest;
pub type OpEnvLoadByPathRequest = OpEnvLoadByPathOutputRequest;
pub type OpEnvCreateNewRequest = OpEnvCreateNewOutputRequest;

pub type OpEnvLockRequest = OpEnvLockOutputRequest;

pub type OpEnvCommitRequest = OpEnvCommitOutputRequest;
pub type OpEnvCommitResponse = OpEnvCommitOutputResponse;

pub type OpEnvAbortRequest = OpEnvAbortOutputRequest;

pub type OpEnvMetadataRequest = OpEnvMetadataOutputRequest;
pub type OpEnvMetadataResponse = OpEnvMetadataOutputResponse;

pub type OpEnvGetByKeyRequest = OpEnvGetByKeyOutputRequest;
pub type OpEnvGetByKeyResponse = OpEnvGetByKeyOutputResponse;

pub type OpEnvInsertWithKeyRequest = OpEnvInsertWithKeyOutputRequest;

pub type OpEnvSetWithKeyRequest = OpEnvSetWithKeyOutputRequest;
pub type OpEnvSetWithKeyResponse = OpEnvSetWithKeyOutputResponse;

pub type OpEnvRemoveWithKeyRequest = OpEnvRemoveWithKeyOutputRequest;
pub type OpEnvRemoveWithKeyResponse = OpEnvRemoveWithKeyOutputResponse;

pub type OpEnvContainsRequest = OpEnvContainsOutputRequest;
pub type OpEnvContainsResponse = OpEnvContainsOutputResponse;

pub type OpEnvInsertRequest = OpEnvInsertOutputRequest;
pub type OpEnvInsertResponse = OpEnvInsertOutputResponse;

pub type OpEnvRemoveRequest = OpEnvRemoveOutputRequest;
pub type OpEnvRemoveResponse = OpEnvRemoveOutputResponse;

pub type RootStateAccessGetObjectByPathRequest = RootStateAccessGetObjectByPathOutputRequest;
pub type RootStateAccessGetObjectByPathResponse = RootStateAccessGetObjectByPathOutputResponse;

pub type RootStateAccessListRequest = RootStateAccessListOutputRequest;
pub type RootStateAccessListResponse = RootStateAccessListOutputResponse;