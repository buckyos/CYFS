use super::dec_app::*;
use cyfs_base::ObjectId;


// get the default dec_app id for all the system service and system core
pub fn get_system_dec_app() -> &'static ObjectId {
    cyfs_base::get_system_dec_app()
}

pub fn is_system_dec_app(dec_id: &Option<ObjectId>) -> bool {
    cyfs_base::is_system_dec_app(dec_id)
}

pub struct SystemDecApp;

impl SystemDecApp {
    pub fn gen_system_dec_id() -> DecAppId {
        let owner = cyfs_base::ObjectId::default();
        DecApp::generate_id(owner, "cyfs-system-service")
            .try_into()
            .unwrap()
    }

    pub fn init_system_dec_id() {
        let dec_id = Self::gen_system_dec_id().into();
        cyfs_base::init_system_dec_app(dec_id);
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
