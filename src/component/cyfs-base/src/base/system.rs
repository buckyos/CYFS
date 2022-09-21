use crate::ObjectId;

use once_cell::sync::OnceCell;

pub(crate) static SYSTEM_DEC_APP: OnceCell<ObjectId> = OnceCell::new();

// get the default dec_app id for all the system service and system core
pub fn get_system_dec_app() -> &'static ObjectId {
    SYSTEM_DEC_APP.get().unwrap()
}

pub fn is_system_dec_app(dec_id: &Option<ObjectId>) -> bool {
    match dec_id {
        Some(id) => {
            id == get_system_dec_app()
        }
        None => {
            true
        }
    }
}

pub fn init_system_dec_app(dec_id: ObjectId) {
    SYSTEM_DEC_APP.get_or_init(|| {
        dec_id
    });
}
