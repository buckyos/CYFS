use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use zone_simulator::*;

fn new_dec(name: &str) -> ObjectId {
    let owner_id = &USER1_DATA.get().unwrap().people_id;

    let dec_id = DecApp::generate_id(owner_id.object_id().to_owned(), name);

    info!(
        "generage test storage dec_id={}, people={}",
        dec_id, owner_id
    );

    dec_id
}

pub async fn test() {
    let device_stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);

    test_storage(&device_stack).await;
}

async fn test_storage(_stack: &SharedCyfsStack) {}


#[test]
fn tes_sort() {
    let mut list = vec!["abd".to_owned(), "a".to_owned(), "".to_owned(), "aed".to_owned()];
    list.sort_by(|left, right| right.cmp(&left));

    println!("{:?}", list);
}
