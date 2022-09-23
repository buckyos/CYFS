use crate::ObjectId;

use once_cell::sync::OnceCell;
use std::borrow::Cow;

pub(crate) static SYSTEM_DEC_APP: OnceCell<ObjectId> = OnceCell::new();
pub(crate) static ANONYMOUS_DEC_APP: OnceCell<ObjectId> = OnceCell::new();

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
    info!("init system-dec-id: {}", dec_id);

    SYSTEM_DEC_APP.get_or_init(|| {
        dec_id
    });
}


// get the default dec_app id for all the unknown service and incoming reqeust
pub fn get_anonymous_dec_app() -> &'static ObjectId {
    ANONYMOUS_DEC_APP.get().unwrap()
}

pub fn is_anonymous_dec_app(dec_id: &Option<ObjectId>) -> bool {
    match dec_id {
        Some(id) => {
            id == get_anonymous_dec_app()
        }
        None => {
            true
        }
    }
}

pub fn init_anonymous_dec_app(dec_id: ObjectId) {
    info!("init anonymous-dec-id: {}", dec_id);

    ANONYMOUS_DEC_APP.get_or_init(|| {
        dec_id
    });
}

pub fn dec_id_to_string(dec_id: &ObjectId) -> Cow<str> {
    if dec_id == get_system_dec_app() {
        Cow::Borrowed("system")
    } else if dec_id == get_anonymous_dec_app() {
        Cow::Borrowed("anonymous")
    } else {
        Cow::Owned(dec_id.to_string())
    }
}