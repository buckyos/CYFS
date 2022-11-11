use cyfs_core::{Text, TextObj};
use crate::DEC_ID;

pub fn new_object(id: &str, header: &str) -> Text {
    Text::build(id, header, "bench")
        .no_create_time()
        .dec_id(DEC_ID.clone())
        .build()
}