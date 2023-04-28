use crate::*;
use std::str::FromStr;

#[test]
fn test_codec() {
    let area = Area::new(1, 2, 1, 0);
    let owner = ObjectId::from_str("5r4MYfFMLwNG8oaBA7tmssaV2Z94sVZUfKXejfC1RMjX").unwrap();
    let unique_id = UniqueId::create_with_hash("test_device".as_bytes());

    let sn_list =
        vec![DeviceId::from_str("5aSixgMAjePp1j5M7ngnU6CN6Do2gitKirviqJswuGVM").unwrap()];

    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let public_key = private_key.public();

    let device = Device::new(
        Some(owner.clone()),
        unique_id,
        vec![],
        sn_list,
        vec![],
        public_key,
        area,
        DeviceCategory::IOSMobile,
    )
    .build();

    let buf = device.to_vec().unwrap();
    println!("device without object_id: {}", hex::encode(&buf));

    {
        let device1 = Device::clone_from_slice(&buf).unwrap();
        assert!(device1.body().as_ref().unwrap().object_id().is_none());
    }

    let mut device_with_body_object_id = device.clone();
    device_with_body_object_id
        .body_mut()
        .as_mut()
        .unwrap()
        .set_object_id(Some(owner.clone()));

    let buf = device_with_body_object_id.to_vec().unwrap();
    println!("device with object_id: {}", hex::encode(&buf));

    {
        let device1 = Device::clone_from_slice(&buf).unwrap();
        assert_eq!(device1.body().as_ref().unwrap().object_id(), &Some(owner));
    }
}


/*
The following code needs to be run with old code that does not support body object_id to test compatibility with the new format
 */
/* 
#[test]
fn test_old_compatibility() {
    let s: &str = "00015a02002f58f47dc0d1fe4800000000765965ba5259c2ca0fb345f95b17ad022415ca9fe3eec3d3c431920102000108010030818902818100ab41eb7c0c74eddb92f3ec7efcbe5c93abd8e5df41866af1dfc1807dbf0731c04920e5bb714e9379c986ab0b0f22153a878ac03d94d558b8402f70da4a8d5f170ef8360b0f713ba6b0c869c94063ae1dbc5d84938be7d892f8138dec9b299bc209ddb2538166b10c806b7bd7da12ba87502cdff77f4f5a2e12a6cca5668dc02502030100010000000000000000000000000000000000000000000000000010fb060e8dc1f2b688c4ee3a6a1e2d8d0700002f58f47dc0d21c00012212204400000000b8b0e69a98ca86a81f12eed2d7457dc6318d9f164b2ec75012af88";
    let buf_without_body_object_id = hex::decode(s).unwrap();
    let buf_with_body_object_id: Vec<u8> = hex::decode("00015a02002f58f47dc0d1fe4800000000765965ba5259c2ca0fb345f95b17ad022415ca9fe3eec3d3c431920102000108010030818902818100ab41eb7c0c74eddb92f3ec7efcbe5c93abd8e5df41866af1dfc1807dbf0731c04920e5bb714e9379c986ab0b0f22153a878ac03d94d558b8402f70da4a8d5f170ef8360b0f713ba6b0c869c94063ae1dbc5d84938be7d892f8138dec9b299bc209ddb2538166b10c806b7bd7da12ba87502cdff77f4f5a2e12a6cca5668dc02502030100010000000000000000000000000000000000000000000000000010fb060e8dc1f2b688c4ee3a6a1e2d8d0704002f58f47dc0d21c00220a204800000000765965ba5259c2ca0fb345f95b17ad022415ca9fe3eec3d3c4319200012212204400000000b8b0e69a98ca86a81f12eed2d7457dc6318d9f164b2ec75012af88").unwrap();

    let device1 = Device::clone_from_slice(&buf_without_body_object_id).unwrap();
    let device2 = Device::clone_from_slice(&buf_with_body_object_id).unwrap();

    let buf1 = device1.to_vec().unwrap();
    let buf = device2.to_vec().unwrap();
    assert_eq!(buf1, buf);

    assert_eq!(buf, buf_without_body_object_id);
    assert_ne!(buf, buf_with_body_object_id);
}
*/