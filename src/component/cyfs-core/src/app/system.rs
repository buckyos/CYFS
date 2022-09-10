use super::dec_app::*;

use once_cell::sync::OnceCell;

pub(crate) static SYSTEM_DEC_APP: OnceCell<DecAppId> = OnceCell::new();

// get the default dec_app id for all the system service and system core
pub fn get_system_dec_app() -> &'static DecAppId {
    SYSTEM_DEC_APP.get_or_init(|| {
        let id = SystemDecApp::gen_system_dec_id();
        info!("init system dec_id as {}", id);
        id
    })
}

pub(crate) struct SystemDecApp {}

impl SystemDecApp {
    pub fn gen_system_dec_id() -> DecAppId {
        let owner = cyfs_base::ObjectId::default();
        DecApp::generate_id(owner, "cyfs-system-service")
            .try_into()
            .unwrap()
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
