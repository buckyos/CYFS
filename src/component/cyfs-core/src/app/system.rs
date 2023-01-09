use super::dec_app::*;
use cyfs_base::ObjectId;


pub struct SystemDecApp;

impl SystemDecApp {
    pub fn gen_system_dec_id() -> DecAppId {
        let owner = cyfs_base::ObjectId::default();
        DecApp::generate_id(owner, "cyfs-system-service")
            .try_into()
            .unwrap()
    }

    pub fn gen_anonymous_dec_id() -> DecAppId {
        let owner = cyfs_base::ObjectId::default();
        DecApp::generate_id(owner, "cyfs-anonymous-service")
            .try_into()
            .unwrap()
    }
}


use once_cell::sync::OnceCell;
use std::borrow::Cow;

pub(crate) static SYSTEM_DEC_APP: OnceCell<ObjectId> = OnceCell::new();
pub(crate) static ANONYMOUS_DEC_APP: OnceCell<ObjectId> = OnceCell::new();

// get the default dec_app id for all the system service and system core
pub fn get_system_dec_app() -> &'static ObjectId {
    SYSTEM_DEC_APP.get_or_init(|| {
        let dec_id = SystemDecApp::gen_system_dec_id().into();
        info!("init system dec app id: {}", dec_id);
        dec_id
    })
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


// get the default dec_app id for all the unknown service and incoming reqeust
pub fn get_anonymous_dec_app() -> &'static ObjectId {
    ANONYMOUS_DEC_APP.get_or_init(|| {
        let dec_id = SystemDecApp::gen_anonymous_dec_id().into();
        info!("init anonymous dec app id: {}", dec_id);
        dec_id
    })
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

pub fn dec_id_to_string(dec_id: &ObjectId) -> Cow<str> {
    if dec_id == get_system_dec_app() {
        Cow::Borrowed("system")
    } else if dec_id == get_anonymous_dec_app() {
        Cow::Borrowed("anonymous")
    } else {
        Cow::Owned(dec_id.to_string())
    }
}

#[test]
fn test() {
    let id = get_system_dec_app().to_owned();
    assert_eq!(
        id.to_string(),
        "9tGpLNncauC9kGhZ7GsztFvVegaKwBXoSDjkxGDHqrn6"
    );
    println!("{}", id);
}
