use crate::*;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub(crate) struct CreateNewParam {
    pub key: String,
    pub content_type: ObjectMapSimpleContentType,
}

#[derive(Debug)]
pub(crate) struct InsertWithKeyParam {
    pub key: String,
    pub value: ObjectId,
}

#[derive(Debug)]
pub(crate) struct SetWithKeyParam {
    pub key: String,
    pub value: ObjectId,
    pub prev_value: Option<ObjectId>,
    pub auto_insert: bool,
}

#[derive(Debug)]
pub(crate) struct RemoveWithKeyParam {
    pub key: String,
    pub prev_value: Option<ObjectId>,
}

#[derive(Debug)]
pub(crate) struct InsertParam {
    pub value: ObjectId,
}

#[derive(Debug)]
pub(crate) struct RemoveParam {
    pub value: ObjectId,
}

// ObjectMap的Map类型的(key, value)状态
#[derive(Debug)]
pub(crate) struct ObjectMapKeyState {
    pub value: Option<ObjectId>,
}

#[derive(Debug)]
pub(crate) struct ObjectMapWriteOpData<P, S>
where
    P: std::fmt::Debug,
    S: std::fmt::Debug,
{
    pub path: String,
    pub param: P,
    pub state: Option<S>,
}

pub(crate) type CreateNewOpData = ObjectMapWriteOpData<CreateNewParam, ObjectMapKeyState>;
pub(crate) type InsertWithKeyOpData = ObjectMapWriteOpData<InsertWithKeyParam, ObjectMapKeyState>;
pub(crate) type SetWithKeyOpData = ObjectMapWriteOpData<SetWithKeyParam, ObjectMapKeyState>;
pub(crate) type RemoveWithKeyOpData = ObjectMapWriteOpData<RemoveWithKeyParam, ObjectMapKeyState>;

pub(crate) type InsertOpData = ObjectMapWriteOpData<InsertParam, bool>;
pub(crate) type RemoveOpData = ObjectMapWriteOpData<RemoveParam, bool>;

#[derive(Debug)]
pub(crate) enum ObjectMapWriteOp {
    CreateNew(CreateNewOpData),
    InsertWithKey(InsertWithKeyOpData),
    SetWithKey(SetWithKeyOpData),
    RemoveWithKey(RemoveWithKeyOpData),

    Insert(InsertOpData),
    Remove(RemoveOpData),
}

pub(crate) struct ObjectMapOpList {
    op_list: Arc<Mutex<Vec<ObjectMapWriteOp>>>,
}

impl ObjectMapOpList {
    pub fn new() -> Self {
        Self {
            op_list: Arc::new(Mutex::new(vec![])),
        }
    }

    pub fn append_op(&self, op: ObjectMapWriteOp) {
        let mut op_list = self.op_list.lock().unwrap();
        op_list.push(op);
    }

    pub fn fetch_all(&self) -> Vec<ObjectMapWriteOp> {
        let mut ret = vec![];
        let mut op_list = self.op_list.lock().unwrap();
        std::mem::swap(&mut ret, &mut op_list);

        ret
    }
}
