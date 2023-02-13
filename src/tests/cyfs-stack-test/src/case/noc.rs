
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use zone_simulator::*;

fn new_dec(name: &str) -> ObjectId {
    let owner_id = &USER1_DATA.get().unwrap().people_id;

    let dec_id = DecApp::generate_id(owner_id.object_id().to_owned(), name);

    info!(
        "generage non_handlers dec_id={}, people={}",
        dec_id, owner_id
    );

    dec_id
}

fn new_object(id: &str) -> Text {
    Text::build(id, "test_noc", "hello!")
        .build()
}


pub async fn test() {
    test_put_objects(1024).await;

    info!("test all noc case success!");
}

async fn test_put_objects(count: usize) {
    let device1 = TestLoader::get_shared_stack(DeviceIndex::User1Device1);

    for i in 0..count {
        let object = new_object(&format!("{}", i));
        let id = object.desc().calculate_id();
        let data = object.to_vec().unwrap();

        let req = NONPutObjectOutputRequest::new_noc(id, data);
        device1.non_service().put_object(req).await.unwrap();
    }
}