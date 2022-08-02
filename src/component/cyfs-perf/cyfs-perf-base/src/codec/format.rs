use serde_json::Value;
use cyfs_base::{ObjectFormatAutoWithSerde, ObjectFormat, FileEncoder, FileDecoder};
use crate::*;

impl ObjectFormatAutoWithSerde for SizeResult {}
impl ObjectFormatAutoWithSerde for TimeResult {}
impl ObjectFormatAutoWithSerde for SpeedResult {}

impl ObjectFormatAutoWithSerde for PerfRequestDesc {}
impl ObjectFormatAutoWithSerde for PerfAccumulationDesc {}
impl ObjectFormatAutoWithSerde for PerfActionItem {}
impl ObjectFormat for PerfActionDesc {
    fn format_json(&self) -> Value {
        serde_json::Map::new().into()
    }
}
impl ObjectFormatAutoWithSerde for PerfActionBody {}
impl ObjectFormatAutoWithSerde for PerfRecordDesc {}

#[test]
fn test() {
    use std::str::FromStr;
    use cyfs_base::{ObjectId, ObjectFormat, BuckyErrorCode, RawFrom, bucky_time_now, NamedObject, FileEncoder};

    let owner = ObjectId::from_str("5r4MYfF8wo73agKvNjPu7ENuJKABYEFDZ4xi6efweF9D").unwrap();
    let dec_id = ObjectId::from_str("9tGpLNnAAYE9Dd4ooNiSjtP5MeL9CNLf9Rxu6AFEc12M").unwrap();

    let mut request = PerfRequest::create(owner, dec_id);
    request = request.add_stat(&PerfRequestItem {
        time: bucky_time_now(),
        spend_time: 100000,
        err: BuckyErrorCode::Ok,
        stat: Some(1024000)
    });

    println!("request: {}", request.format_json().to_string());

    let mut acc = PerfAccumulation::create(owner, dec_id);
    acc = acc.add_stats(vec![PerfAccumulationItem {
        time: bucky_time_now(),
        err: BuckyErrorCode::Ok,
        stat: Some(1024000)
    }, PerfAccumulationItem {
        time: bucky_time_now(),
        err: BuckyErrorCode::Ok,
        stat: Some(512)
    }].as_slice());

    println!("acc: {}", acc.format_json().to_string());

    let mut action = PerfAction::create(owner, dec_id);
    action = action.add_stat(PerfActionItem::create(BuckyErrorCode::Ok, "key1".to_owned(), "value1".to_owned()));
    println!("action: {}", action.format_json().to_string());

    action = action.add_stat(PerfActionItem::create(BuckyErrorCode::InvalidData, "key2".to_owned(), "value2".to_owned()));
    println!("action_err: {}", action.format_json().to_string());

    let record = PerfRecord::create(owner, dec_id, 100, Some(1000));
    println!("record: {}", record.format_json().to_string());

    let record_none = PerfRecord::create(owner, dec_id, 100, None);
    println!("record_none: {}", record_none.format_json().to_string());

    println!("test actions over 64k enocde");

    let mut action_items = vec![];
    for i in 0..1000 {
        action_items.push(PerfActionItem::create(
            if i % 2 == 1 {BuckyErrorCode::InvalidData}else { BuckyErrorCode::Ok },
            format!("key{}", i),
            format!("value{}", i)));
    }

    let mut big_action = PerfAction::create(owner, dec_id);

    loop {
        big_action = big_action.add_stats(&mut action_items.clone());

        let big_buf = big_action.encode_to_vec(false).unwrap();
        let mut big_action2 = PerfAction::clone_from_slice(&big_buf).unwrap();
        println!("decode big action, {} actions", big_action2.body().as_ref().unwrap().content().actions.len());

        if big_buf.len() > 1024*64 {
            println!("big actions over 64k limit, {} actions", big_action.body().as_ref().unwrap().content().actions.len());
            break;
        } else {
            println!("big actions not over 64k limit, {} actions", big_action.body().as_ref().unwrap().content().actions.len());
        }
    }
}