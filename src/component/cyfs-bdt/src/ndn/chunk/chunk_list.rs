use std::{
    collections::{BTreeMap},
    str::FromStr,
    ops::Range
};
use async_std::{sync::Arc};
use generic_array::{
    typenum::{marker_traits::Unsigned, U32},
    GenericArray,
};
use cyfs_base::*;


#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChunkListId(GenericArray<u8, U32>);

impl Default for ChunkListId {
    fn default() -> Self {
        Self(GenericArray::default())
    }
}

impl ChunkListId {
    pub fn new(ar: GenericArray<u8, U32>) -> Self {
        Self::from(ar)
    }
    pub fn to_string(&self) -> String {
        hex::encode(self.0.as_slice())
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }
}

impl From<GenericArray<u8, U32>> for ChunkListId {
    fn from(hash: GenericArray<u8, U32>) -> Self {
        Self(hash)
    }
}

impl std::fmt::Debug for ChunkListId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl std::fmt::Display for ChunkListId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl FromStr for ChunkListId {
    type Err = BuckyError;
    fn from_str(s: &str) -> BuckyResult<Self> {
        match hex::decode(s) {
            Ok(v) => {
                if v.len() != 32 {
                    let msg = format!(
                        "invalid TransSessionId string length: {}, len={}",
                        s,
                        v.len()
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                }

                let r = GenericArray::clone_from_slice(&v);
                Ok(Self(r))
            }
            Err(e) => {
                let msg = format!("invalid TransSessionId string hex format: {} {}", s, e);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
            }
        }
    }
}

impl AsRef<GenericArray<u8, U32>> for ChunkListId {
    fn as_ref(&self) -> &GenericArray<u8, U32> {
        &self.0
    }
}

impl RawFixedBytes for ChunkListId {
    fn raw_bytes() -> Option<usize> {
        Some(U32::to_usize())
    }
}

impl RawEncode for ChunkListId {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        Ok(U32::to_usize())
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!("not enough buffer for encode TransSessionId, except={}, got={}", bytes, buf.len());
            error!("{}", msg);

            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                msg,
            ));
        }
        unsafe {
            std::ptr::copy(self.as_slice().as_ptr(), buf.as_mut_ptr(), bytes);
        }

        Ok(&mut buf[bytes..])
    }
}

impl<'de> RawDecode<'de> for ChunkListId {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!("not enough buffer for decode TransSessionId, except={}, got={}", bytes, buf.len());
            error!("{}", msg);

            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                msg,
            ));
        }
        let mut _id = Self::default();
        unsafe {
            std::ptr::copy(buf.as_ptr(), _id.as_mut_slice().as_mut_ptr(), bytes);
        }
        Ok((_id, &buf[bytes..]))
    }
}


struct ChunkListDescImpl {
    chunks: Vec<ChunkId>,
    offsets: Vec<u64>,
    total_len: u64,
    index_map: BTreeMap<ChunkId, Vec<usize>>, 
}

#[derive(Clone)]
pub struct ChunkListDesc(Arc<ChunkListDescImpl>);

impl Default for ChunkListDesc {
    fn default() -> Self {
        Self(Arc::new(ChunkListDescImpl {
            chunks: vec![],
            offsets: vec![],
            index_map: BTreeMap::new(),
            total_len: 0,
        }))
    }
}

impl ChunkListDesc {
    pub fn from_chunk(chunk: ChunkId) -> Self {
        let mut index_map = BTreeMap::new();
        index_map.insert(chunk.clone(), vec![0]);
        let desc = ChunkListDescImpl {
            total_len: chunk.len() as u64,
            chunks: vec![chunk],
            offsets: vec![0],
            index_map,
        };

        Self(Arc::new(desc))
    }

    pub fn from_chunks(chunk_list: &Vec<ChunkId>) -> Self {
        let mut total_len = 0u64;
        let mut offsets = vec![0u64; chunk_list.len()];
        let mut chunks = vec![];
        for (index, chunk) in chunk_list.into_iter().enumerate() {
            offsets[index] = total_len;
            total_len += chunk.len() as u64;
            chunks.push(chunk.clone())
        }

        let mut desc = ChunkListDescImpl {
            chunks,
            offsets,
            total_len,
            index_map: BTreeMap::new(),
        };

        for (index, chunk) in desc.chunks.iter().enumerate() {
            if let Some(exists) = desc.index_map.get_mut(chunk) {
                exists.push(index);
            } else {
                desc.index_map.insert(chunk.clone(), vec![index]);
            }
        }

        Self(Arc::new(desc))
    }

    pub fn from_file(file: &File) -> BuckyResult<Self> {
        match file.body() {
            Some(body) => {
                let chunk_list = body.content().inner_chunk_list();
                match chunk_list {
                    Some(list) => {
                        Ok(Self::from_chunks(list))
                    }
                    None => Err(BuckyError::new(
                        BuckyErrorCode::NotSupport,
                        format!("file object should has chunk list: {}", file.desc().calculate_id()),
                    )),
                }
            }
            None => {
                Err(BuckyError::new(
                    BuckyErrorCode::InvalidFormat,
                    format!("file object should has body: {}", file.desc().calculate_id()),
                ))
            }
        }
    }

    pub fn chunks(&self) -> &[ChunkId] {
        self.0.chunks.as_slice()
    }

    pub fn index_of(&self, chunk: &ChunkId) -> Option<&Vec<usize>> {
        self.0.index_map.get(chunk)
    }

    pub fn offset_of(&self, index: usize) -> Option<u64> {
        self.0.offsets.get(index).map(|o| *o)
    }

    pub fn total_len(&self) -> u64 {
        self.0.total_len
    }

    pub fn range_of(&self, range: Range<u64>) -> Vec<(usize, Range<u64>)> {
        let mut ranges = vec![];
        let mut start = range.start;
        let end = range.end;
        for (index, chunk) in self.chunks().iter().enumerate() {
            let offset = self.offset_of(index).unwrap();
            let next_offset = offset + chunk.len() as u64;
            if offset <= start 
                && next_offset > start {
                if next_offset >= end {
                    ranges.push((index, start - offset..end - offset));
                    break;
                } else {
                    ranges.push((index, start - offset..chunk.len() as u64));
                    start = next_offset;
                }
            }
        }

        ranges
    }
}



#[derive(Clone, Debug)]
pub struct ChunkStateList {
    start: usize,
    states: Vec<u8>,
}

impl Default for ChunkStateList {
    fn default() -> Self {
        Self {
            start: 0,
            states: vec![],
        }
    }
}

impl From<(usize, Vec<ChunkState>)> for ChunkStateList {
    fn from(params: (usize, Vec<ChunkState>)) -> Self {
        let (start, states) = params;
        Self {
            start,
            states: Vec::from_iter(states.into_iter().map(|s| s.into())),
        }
    }
}

impl ChunkStateList {
    pub fn new(desc: &ChunkListDesc) -> Self {
        Self {
            start: 0,
            states: vec![ChunkState::Unknown.into(); desc.chunks().len()],
        }
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn range(&self) -> Range<usize> {
        self.start..(self.start + self.len())
    }

    pub fn state_of_index(&self, index: usize) -> Option<ChunkState> {
        self.states
            .get(index - self.start)
            .map(|s| ChunkState::try_from(*s).unwrap())
    }

    pub fn state_of(&self, desc: &ChunkListDesc, chunk: &ChunkId) -> Option<ChunkState> {
        desc.index_of(chunk).map(|indices| {
            self.states
                .get(indices[0] - self.start)
                .map(|s| ChunkState::try_from(*s).unwrap())
                .unwrap()
        })
    }

    pub fn update_of(
        &mut self,
        desc: &ChunkListDesc,
        chunk: &ChunkId,
        state: ChunkState,
    ) -> Option<ChunkState> {
        desc.index_of(chunk).map(|indices| {
            for i in indices {
                *self.states.get_mut(i - self.start).unwrap() = state.into();
            }
            state
        })
    }

    pub fn update_index_of(&mut self, index: usize, state: ChunkState) -> Option<ChunkState> {
        let index = index - self.start;
        let former = ChunkState::try_from(self.states[index]).unwrap();
        self.states[index] = state as u8;
        Some(former)
    }

    pub fn range_of(&self, range: &Range<usize>) -> Self {
        let start = range.start;
        let states = Vec::from(&self.states.as_slice()[(range.start - self.start)..(range.end - self.start)]);
        Self { start, states }
    }

    pub fn update_range_of(&mut self, range: &Self) -> usize {
        let r = range.range();
        self.states.as_mut_slice()[(r.start - self.start)..(r.end - self.start)].copy_from_slice(range.states.as_slice());
        range.len()
    }

    pub fn is_range_ready(&self, range: &Range<usize>) -> Option<Self> {
        for state in &self.states.as_slice()[(range.start - self.start)..(range.end - self.start)] {
            if *state == ChunkState::Ready as u8 {
                return Some(self.range_of(range));
            }
        }
        None
    }

    pub fn range_ready_count(&self, range: &Range<usize>) -> usize {
        let mut count = 0;
        for state in &self.states.as_slice()[(range.start - self.start)..(range.end - self.start)] {
            if *state == ChunkState::Ready as u8 {
                count += 1;
            }
        }
        count
    }
}

impl RawEncode for ChunkStateList {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        Ok(u32::raw_bytes().unwrap() 
            + u32::raw_bytes().unwrap()
            + self.states.len())
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let bytes = self.raw_measure(purpose)?;
        if buf.len() < bytes {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer",
            ));
        }
        let buf = (self.start as u32).raw_encode(buf, purpose)?;
        let buf = (self.states.len() as u32).raw_encode(buf, purpose)?;
        unsafe {
            std::ptr::copy(
                self.states.as_slice().as_ptr(),
                buf.as_mut_ptr(),
                self.states.len(),
            );
        }
        Ok(&mut buf[self.states.len()..])
    }
}

impl<'de> RawDecode<'de> for ChunkStateList {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        if buf.len() < u32::raw_bytes().unwrap() {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer",
            ));
        }
        let (start, buf) = u32::raw_decode(buf)?;
        let start = start as usize;

        if buf.len() < u32::raw_bytes().unwrap() {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer",
            ));
        }
        let (len, buf) = u32::raw_decode(buf)?;
        let len = len as usize;

        if buf.len() < len {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer",
            ));
        }
        let mut states = vec![0u8; len];
        unsafe {
            std::ptr::copy(buf.as_ptr(), states.as_mut_slice().as_mut_ptr(), len);
        }

        let bytes = Self { start, states };

        Ok((bytes, &buf[len..]))
    }
}
