use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut, Range};

/*
Remember that the range is zero-indexed, so Range: bytes=0-999 is actually requesting 1000 bytes, not 999, so respond with something like:

Content-Length: 1000
Content-Range: bytes 0-999/123456
*/

#[derive(Clone)]
pub struct NDNDataRange {
    pub start: Option<u64>,
    pub length: Option<u64>,
}

impl NDNDataRange {
    // https://developer.mozilla.org/zh-CN/docs/Web/HTTP/Headers/Range
    pub fn to_string(&self) -> String {
        let start = self.start.unwrap_or(0);
        match self.length {
            Some(len) => {
                format!("{}-{}", start, start + len - 1)
            }
            None => {
                format!("{}-", start)
            }
        }
    }
}

impl From<Range<u64>> for NDNDataRange {
    fn from(v: Range<u64>) -> Self {
        let length = if v.end >= v.start {
            v.end - v.start
        } else {
            0
        };

        Self {
            start: Some(v.start),
            length: Some(length),
        }
    }
}

#[derive(Clone)]
pub struct NDNDataRanges(Vec<NDNDataRange>);

impl Deref for NDNDataRanges {
    type Target = Vec<NDNDataRange>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for NDNDataRanges {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}


impl NDNDataRanges {
    pub fn ranges(&self) -> &Vec<NDNDataRange> {
        &self.0
    }

    // Range: <unit>=<range-start>-<range-end>, <range-start>-<range-end>
    pub fn to_string(&self) -> String {
        if self.0.len() == 0 {
            "bytes=0-".to_owned()
        } else {
            let ranges: Vec<String> = self.0.iter().map(|range| range.to_string()).collect();

            format!("bytes={}", ranges.join(", "))
        }
    }
}

#[derive(Clone)]
pub enum NDNDataRequestRange {
    Unparsed(String),
    Range(NDNDataRanges),
}

impl std::fmt::Display for NDNDataRequestRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_display_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NDNDataResponseRange {
    NoOverlap(u64),
    InvalidRange,
    Range((Vec<Range<u64>>, u64)),
}

impl ObjectFormatAutoWithSerde for NDNDataResponseRange {}
impl JsonCodecAutoWithSerde for NDNDataResponseRange {}

impl NDNDataRequestRange {
    pub fn new_data_range(ranges: Vec<NDNDataRange>) -> Self {
        Self::Range(NDNDataRanges(ranges))
    }

    pub fn new_range(ranges: Vec<Range<u64>>) -> Self {
        let ranges = ranges.into_iter().map(|v| {
            v.into()
        }).collect();

        Self::new_data_range(ranges)
    }

    pub fn new_unparsed(s: impl ToString) -> Self {
        Self::Unparsed(s.to_string())
    }

    pub fn encode_string(&self) -> String {
        match self {
            Self::Unparsed(s) => s.clone(),
            Self::Range(range) => range.to_string(),
        }
    }

    pub fn to_display_string(&self) -> String {
        match self {
            Self::Unparsed(s) => {
                format!("unparsed: {}", s)
            }
            Self::Range(range) => range.to_string(),
        }
    }

    pub fn convert_to_response(&self, size: u64) -> Option<NDNDataResponseRange> {
        let ret = match self {
            Self::Unparsed(s) => Self::parse_str(s.as_str(), size),
            Self::Range(list) => Self::parse_ranges(list, size),
        };

        info!("data range convert: {} -> {:?}", self, ret);

        ret
    }

    fn parse_ranges(list: &NDNDataRanges, size: u64) -> Option<NDNDataResponseRange> {
        let mut no_overlap = false;
        let mut ranges = vec![];
        for range in list.ranges() {
            match Self::parse_range(range, size) {
                Some(v) => match v {
                    NDNDataResponseRange::Range((range, _)) => {
                        assert!(range.len() > 0);
                        ranges.extend_from_slice(&range);
                    }
                    NDNDataResponseRange::InvalidRange => {
                        return Some(NDNDataResponseRange::InvalidRange);
                    }
                    NDNDataResponseRange::NoOverlap(_) => {
                        unreachable!();
                    }
                },
                None => {
                    no_overlap = true;
                    continue;
                }
            }
        }

        if ranges.is_empty() {
            if no_overlap {
                return Some(NDNDataResponseRange::NoOverlap(size));
            } else {
                return None;
            }
        }

        Some(NDNDataResponseRange::Range((ranges, size)))
    }

    fn parse_range(range: &NDNDataRange, size: u64) -> Option<NDNDataResponseRange> {
        match range.start {
            Some(start) => {
                if start >= size {
                    return None;
                }

                match range.length {
                    Some(mut len) => {
                        if len == 0 {
                            None
                        } else {
                            if start + len > size {
                                len = size - start;
                            }
                            let range = Range {
                                start,
                                end: start + len,
                            };
                            Some(NDNDataResponseRange::Range((vec![range], size)))
                        }
                    }
                    None => {
                        let len = size - start;
                        let range = Range {
                            start,
                            end: start + len,
                        };
                        Some(NDNDataResponseRange::Range((vec![range], size)))
                    }
                }
            }
            // treat start as 0
            None => match range.length {
                Some(mut len) => {
                    if len > size {
                        len = size;
                    }

                    let range = Range { start: 0, end: len };
                    Some(NDNDataResponseRange::Range((vec![range], size)))
                }
                None => Some(NDNDataResponseRange::InvalidRange),
            },
        }
    }

    fn parse_str(range_str: &str, size: u64) -> Option<NDNDataResponseRange> {
        let ranges = match http_range::HttpRange::parse(range_str, size) {
            Ok(r) => r,
            Err(err) => {
                let res = match err {
                    http_range::HttpRangeParseError::NoOverlap => {
                        NDNDataResponseRange::NoOverlap(size)
                    }
                    http_range::HttpRangeParseError::InvalidRange => {
                        NDNDataResponseRange::InvalidRange
                    }
                };

                return Some(res);
            }
        };

        if ranges.is_empty() {
            return None;
        }

        let ranges: Vec<Range<u64>> = ranges
            .into_iter()
            .map(|item| Range {
                start: item.start,
                end: item.start + item.length,
            })
            .collect();

        Some(NDNDataResponseRange::Range((ranges, size)))
    }
}

pub struct RangeHelper<Idx> {
    _marker: std::marker::PhantomData<Idx>,
}

impl<Idx> RangeHelper<Idx> {
    pub fn intersect(left: &Range<Idx>, right: &Range<Idx>) -> Option<Range<Idx>>
    where
        Idx: PartialOrd<Idx> + Ord + Copy,
    {
        if left.end <= right.start {
            None
        } else if left.start >= right.end {
            None
        } else {
            let start = std::cmp::max(left.start, right.start);
            let end = std::cmp::min(left.end, right.end);

            let r = Range { start, end };
            if r.is_empty() {
                None
            } else {
                Some(r)
            }
        }
    }

    pub fn intersect_list(target: &Range<Idx>, ranges: &Vec<Range<Idx>>) -> Vec<Range<Idx>>
    where
        Idx: PartialOrd<Idx> + Ord + Copy,
    {
        ranges
            .iter()
            .filter_map(|range| Self::intersect(target, range))
            .collect()
    }

    pub fn sum(ranges: &Vec<Range<Idx>>) -> Idx
    where
        Idx: std::ops::Add<Output = Idx> + std::ops::Sub<Output = Idx> + Default + Copy,
    {
        ranges
            .iter()
            .fold(Idx::default(), |acc, v| acc + (v.end - v.start))
    }
}

use super::requestor_helper::RequestorHelper;

pub struct RequestorRangeHelper {}

impl RequestorRangeHelper {
    // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Range
    fn encode_content_range(range: &Vec<Range<u64>>, size: u64) -> String {
        assert!(range.len() > 0);
        if range.len() > 1 {
            warn!(
                "only single range support for http range protocol! ranges={:?}, size={}",
                range, size
            );
        }

        let range = &range[0];

        if size > 0 {
            format!("bytes {}-{}/{}", range.start, range.end - 1, size)
        } else {
            format!("bytes {}-{}/*", range.start, range.end - 1)
        }
    }

    fn encode_empty_range(size: u64) -> String {
        format!("bytes */{}", size)
    }

    pub fn new_range_response(range_resp: &NDNDataResponseRange) -> http_types::Response {
        let mut resp = match range_resp {
            NDNDataResponseRange::Range((ranges, len)) => {
                let mut resp =
                    RequestorHelper::new_response(http_types::StatusCode::PartialContent);

                let value = Self::encode_content_range(ranges, *len);
                resp.insert_header(http_types::headers::CONTENT_RANGE, value);

                resp
            }
            NDNDataResponseRange::NoOverlap(len) => {
                let msg = format!("Invalid range, no overlap! size={}", len);
                let e = BuckyError::new(BuckyErrorCode::RangeNotSatisfiable, msg);

                let mut resp: http_types::Response = RequestorHelper::trans_error(e);

                let value = Self::encode_empty_range(*len);
                resp.insert_header(http_types::headers::CONTENT_RANGE, value);

                resp
            }
            NDNDataResponseRange::InvalidRange => {
                let e = BuckyError::new(BuckyErrorCode::RangeNotSatisfiable, "Invalid range");

                RequestorHelper::trans_error(e)
            }
        };

        resp.insert_header(http_types::headers::ACCEPT_RANGES, "bytes");

        resp
    }
}
