mod processor;
mod transform;

pub(crate) use processor::*;
pub(crate) use transform::*;

use cyfs_base::ObjectId;

pub struct TaskGroupHelper;

impl TaskGroupHelper {
    pub fn new_opt_with_dec(dec_id: &ObjectId, group: Option<&str>) -> Option<String> {
        match group {
            Some(v) => {
                Some(Self::new_with_dec(dec_id, v))
            }
            None => {
                None
            }
        }
    }

    pub fn new_with_dec(dec_id: &ObjectId, group: &str) -> String {
        format!("{}/{}", dec_id, group.trim_start_matches('/'))
    }
}