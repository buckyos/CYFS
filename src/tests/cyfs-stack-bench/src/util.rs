use cyfs_core::{Text, TextObj};
use crate::DEVICE_DEC_ID;

pub fn new_object(id: &str, header: &str) -> Text {
    Text::build(id, header, "bench")
        .no_create_time()
        .dec_id(DEVICE_DEC_ID.clone())
        .build()
}