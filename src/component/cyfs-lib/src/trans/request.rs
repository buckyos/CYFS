use super::output_request::*;

pub type TransGetContextRequest = TransGetContextOutputRequest;
pub type TransGetContextResponse = TransGetContextOutputResponse;

pub type TransPutContextRequest = TransPutContextOutputRequest;

pub type TransCreateTaskRequest =TransCreateTaskOutputRequest;
pub type TransCreateTaskResponse = TransCreateTaskOutputResponse;

pub type TransControlTaskRequest = TransControlTaskOutputRequest;

pub type TransGetTaskStateRequest = TransGetTaskStateOutputRequest;
pub type TransGetTaskStateResponse = TransGetTaskStateOutputResponse;

pub type TransQueryTasksRequest = TransQueryTasksOutputRequest;
pub type TransQueryTasksResponse = TransQueryTasksOutputResponse;

pub type TransPublishFileRequest = TransPublishFileOutputRequest;
pub type TransPublishFileResponse = TransPublishFileOutputResponse;

pub type TransGetTaskGroupStateRequest = TransGetTaskGroupStateOutputRequest;
pub type TransGetTaskGroupStateResponse = TransGetTaskGroupStateOutputResponse;

pub type TransControlTaskGroupRequest = TransControlTaskGroupOutputRequest;
pub type TransControlTaskGroupResponse = TransControlTaskGroupOutputResponse;