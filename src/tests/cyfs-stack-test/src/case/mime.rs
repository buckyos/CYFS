use cyfs_base::*;
use cyfs_lib::*;
use zone_simulator::*;

const SVG_IMAGE: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
<svg width="391" height="391" viewBox="-70.5 -70.5 391 391" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
<rect fill="#fff" stroke="#000" x="-70" y="-70" width="390" height="390"/>
<g opacity="0.8">
	<rect x="25" y="25" width="200" height="200" fill="lime" stroke-width="4" stroke="pink" />
	<circle cx="125" cy="125" r="75" fill="orange" />
	<polyline points="50,150 50,200 200,200 200,100" stroke="red" stroke-width="4" fill="none" />
	<line x1="50" y1="50" x2="200" y2="200" stroke="blue" stroke-width="4" />
</g>
</svg>"##;

pub async fn test() {
    add_svg().await;
    
    info!("test all mime test case success!");
}

async fn add_svg() {
    let owner_id = &USER1_DATA.get().unwrap().people_id;
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);

    let data = SVG_IMAGE.as_bytes().to_owned();
    let chunk_id = ChunkId::calculate_sync(SVG_IMAGE.as_bytes()).unwrap();
    let req = NDNPutDataOutputRequest::new_with_buffer(
        NDNAPILevel::NDC,
        chunk_id.object_id(),
        data.clone(),
    );

    stack.ndn_service().put_data(req).await.unwrap();

    let hash = hash_data(&data);
    let chunk_list = ChunkList::ChunkInList(vec![chunk_id.clone()]);
    let file = File::new(owner_id.object_id().clone(), data.len() as u64, hash, chunk_list)
        .no_create_time()
        .build();

    let file_id = file.desc().calculate_id();
    info!(
        "svg file={}, chunk={}, len={}",
        file_id,
        chunk_id,
        data.len()
    );

    let mut req = NONPutObjectRequest::new_noc(file_id.clone(), file.to_vec().unwrap());
    req.access = Some(AccessString::full_except_write());
    stack.non_service().put_object(req).await.unwrap();

    info!(
        "put svg file object to local noc success! file={}, owner={}",
        file_id, owner_id
    );
}

#[test] 
fn test_int() {
    let v = std::sync::atomic::AtomicU16::new(u16::MAX - 100);
    for i in 0..1000 {
        let ret = v.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        println!("{}", ret);
    }

    let mut v: u16 = u16::MAX -1;
    v += 1;
}