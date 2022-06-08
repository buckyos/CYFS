use super::protos;
use crate::items::*;
use cyfs_base::*;

use std::collections::HashMap;
use std::convert::TryFrom;

// PerfRequest的编解码
impl TryFrom<protos::PerfTimeRange> for PerfTimeRange {
    type Error = BuckyError;

    fn try_from(value: protos::PerfTimeRange) -> BuckyResult<Self> {
        let ret = Self {
            begin: ProtobufCodecHelper::decode_str_value(value.get_begin())?,
            end: ProtobufCodecHelper::decode_str_value(value.get_end())?,
        };

        Ok(ret)
    }
}

impl TryFrom<&PerfTimeRange> for protos::PerfTimeRange {
    type Error = BuckyError;

    fn try_from(value: &PerfTimeRange) -> BuckyResult<Self> {
        let mut ret = Self::new();

        ret.set_begin(value.begin.to_string());
        ret.set_end(value.end.to_string());

        Ok(ret)
    }
}

// PerfRequest的编解码
impl TryFrom<protos::PerfRequest> for PerfRequest {
    type Error = BuckyError;

    fn try_from(mut value: protos::PerfRequest) -> BuckyResult<Self> {
        let mut ret = Self {
            time_range: PerfTimeRange::try_from(value.take_time_range())?,
            id: value.take_id(),
            total: value.get_total(),
            success: value.get_success(),
            total_time: ProtobufCodecHelper::decode_str_value(value.get_total_time())?,
            total_size: None,
        };

        if value.has_total_size() {
            ret.total_size = Some(ProtobufCodecHelper::decode_str_value(
                value.get_total_size(),
            )?);
        }

        Ok(ret)
    }
}

impl TryFrom<&PerfRequest> for protos::PerfRequest {
    type Error = BuckyError;

    fn try_from(value: &PerfRequest) -> BuckyResult<Self> {
        let mut ret = Self::new();

        let time_range = (&value.time_range).try_into().unwrap();

        ret.set_id(value.id.clone());
        ret.set_time_range(time_range);
        ret.set_total(value.total.clone());
        ret.set_success(value.success.clone());
        ret.set_total_time(value.total_time.to_string());
        if let Some(total_size) = &value.total_size {
            ret.set_total_size(total_size.to_string());
        }
        Ok(ret)
    }
}

// PerfAccumulation的编解码
impl TryFrom<protos::PerfAccumulation> for PerfAccumulation {
    type Error = BuckyError;

    fn try_from(mut value: protos::PerfAccumulation) -> BuckyResult<Self> {
        let mut ret = Self {
            id: value.take_id(),
            time_range: PerfTimeRange::try_from(value.take_time_range())?,
            total: value.get_total(),
            success: value.get_success(),
            total_size: None,
        };

        if value.has_total_size() {
            ret.total_size = Some(ProtobufCodecHelper::decode_str_value(
                value.get_total_size(),
            )?);
        }

        Ok(ret)
    }
}

impl TryFrom<&PerfAccumulation> for protos::PerfAccumulation {
    type Error = BuckyError;

    fn try_from(value: &PerfAccumulation) -> BuckyResult<Self> {
        let mut ret = Self::new();

        let time_range = (&value.time_range).try_into().unwrap();

        ret.set_id(value.id.clone());
        ret.set_time_range(time_range);
        ret.set_success(value.success.clone());
        ret.set_total(value.total.clone());

        if let Some(total_size) = &value.total_size {
            ret.set_total_size(total_size.to_string());
        }
        Ok(ret)
    }
}

// PerfRecord的编解码
impl TryFrom<protos::PerfRecord> for PerfRecord {
    type Error = BuckyError;

    fn try_from(mut value: protos::PerfRecord) -> BuckyResult<Self> {
        let mut ret = Self {
            id: value.take_id(),
            time: ProtobufCodecHelper::decode_str_value(value.get_time())?,
            total: ProtobufCodecHelper::decode_str_value(value.get_total())?,
            total_size: None,
        };

        if value.has_total_size() {
            ret.total_size = Some(ProtobufCodecHelper::decode_str_value(
                value.get_total_size(),
            )?);
        }

        Ok(ret)
    }
}

impl TryFrom<&PerfRecord> for protos::PerfRecord {
    type Error = BuckyError;

    fn try_from(value: &PerfRecord) -> BuckyResult<Self> {
        let mut ret = Self::new();
        ret.set_id(value.id.clone());
        ret.set_time(value.time.to_string());
        ret.set_total(value.total.to_string());

        if let Some(total_size) = &value.total_size {
            ret.set_total_size(total_size.to_string());
        }

        Ok(ret)
    }
}

// PerfAction的编解码
impl TryFrom<protos::PerfAction> for PerfAction {
    type Error = BuckyError;

    fn try_from(mut value: protos::PerfAction) -> BuckyResult<Self> {
        Ok(Self {
            id: value.take_id(),
            time: ProtobufCodecHelper::decode_str_value(value.get_time())?,
            err: value.get_err(),
            name: value.get_name().to_string(),
            value: value.get_value().to_string(),
        })
    }
}

impl TryFrom<&PerfAction> for protos::PerfAction {
    type Error = BuckyError;

    fn try_from(value: &PerfAction) -> BuckyResult<Self> {
        let mut ret = Self::new();
        ret.set_id(value.id.clone());
        ret.set_time(value.time.to_string());
        ret.set_err(value.err.clone());
        ret.set_name(value.name.clone());
        ret.set_value(value.value.clone());
        Ok(ret)
    }
}

// PerfIsolateEntitye的编解码
impl TryFrom<protos::PerfIsolateEntity> for PerfIsolateEntity {
    type Error = BuckyError;

    fn try_from(mut value: protos::PerfIsolateEntity) -> BuckyResult<Self> {
        let id = value.take_id();

        let time_range = PerfTimeRange::try_from(value.take_time_range())?;

        let mut actions = Vec::new();
        for item in value.take_actions() {
            actions.push(PerfAction::try_from(item)?);
        }

        let mut reqs = HashMap::new();
        for (k, v) in value.take_reqs() {
            reqs.insert(k, PerfRequest::try_from(v)?);
        }

        let mut records = HashMap::new();
        for (k, v) in value.take_records() {
            records.insert(k, PerfRecord::try_from(v)?);
        }

        let mut accumulations = HashMap::new();
        for (k, v) in value.take_accumulations() {
            accumulations.insert(k, PerfAccumulation::try_from(v)?);
        }

        Ok(Self {
            id,
            time_range,
            actions,
            records,
            accumulations,
            reqs,
        })
    }
}

impl TryFrom<&PerfIsolateEntity> for protos::PerfIsolateEntity {
    type Error = BuckyError;

    fn try_from(value: &PerfIsolateEntity) -> BuckyResult<Self> {
        let mut ret = Self::new();

        let time_range = (&value.time_range).try_into().unwrap();

        let mut list = Vec::new();
        for action in &value.actions {
            list.push(ProtobufCodecHelper::encode_nested_item(action)?);
        }

        let mut records = HashMap::new();
        for (k, v) in &value.records {
            records.insert(k.to_owned(), ProtobufCodecHelper::encode_nested_item(v)?);
        }

        let mut accumulations = HashMap::new();
        for (k, v) in &value.accumulations {
            accumulations.insert(k.to_owned(), ProtobufCodecHelper::encode_nested_item(v)?);
        }

        let mut reqs = HashMap::new();
        for (k, v) in &value.reqs {
            reqs.insert(k.to_owned(), ProtobufCodecHelper::encode_nested_item(v)?);
        }

        ret.set_id(value.id.clone());
        ret.set_time_range(time_range);
        ret.set_actions(list.into());
        ret.set_records(records);
        ret.set_accumulations(accumulations);
        ret.set_reqs(reqs);

        Ok(ret)
    }
}
