use cyfs_base::{ObjectFormatAutoWithSerde};
use crate::*;

impl ObjectFormatAutoWithSerde for SizeResult {}
impl ObjectFormatAutoWithSerde for TimeResult {}
impl ObjectFormatAutoWithSerde for SpeedResult {}

impl ObjectFormatAutoWithSerde for PerfRequestDesc {}
impl ObjectFormatAutoWithSerde for PerfAccumulationDesc {}
impl ObjectFormatAutoWithSerde for PerfActionDesc {}
impl ObjectFormatAutoWithSerde for PerfRecordDesc {}

#[test]
fn test() {
    use std::str::FromStr;
    use cyfs_base::{ObjectId, ObjectFormat, BuckyErrorCode};

    let owner = ObjectId::from_str("5r4MYfF8wo73agKvNjPu7ENuJKABYEFDZ4xi6efweF9D").unwrap();
    let dec_id = ObjectId::from_str("9tGpLNnAAYE9Dd4ooNiSjtP5MeL9CNLf9Rxu6AFEc12M").unwrap();
    let mut request = PerfRequest::create(owner, dec_id);
    request = request.add_stat(&PerfRequestItem {
        spend_time: 100000,
        err: BuckyErrorCode::Ok,
        stat: Some(1024000)
    });

    println!("request: {}", request.format_json().to_string());

    let mut acc = PerfAccumulation::create(owner, dec_id);
    acc = acc.add_stats(vec![PerfAccumulationItem {
        err: BuckyErrorCode::Ok,
        stat: 1024000
    }, PerfAccumulationItem {
        err: BuckyErrorCode::Ok,
        stat: 512
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
}