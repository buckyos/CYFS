use std::{
    sync::Arc, 
    convert::TryFrom, 
    ops::Range, 
    collections::LinkedList
};
use async_std::{
    io::{prelude::*, Cursor}, 
};
use async_trait::async_trait;
use async_recursion::async_recursion;
use sha2::{Digest, Sha256};
use generic_array::typenum::{marker_traits::Unsigned};
use aes::{Aes256, BlockCipher};
use block_modes::block_padding::NoPadding;
use block_modes::{BlockMode, Cbc};
use cyfs_base::*;
use cyfs_bdt::*;
use cyfs_lib::*;
use crate::{
    protos, 
    obj_id, 
    proof::DsgChallengeSample
};


#[derive(Clone)]
pub struct DsgStackChunkReader(Arc<SharedCyfsStack>);

impl DsgStackChunkReader {
    pub fn new(stack: Arc<SharedCyfsStack>) -> Self {
        Self(stack.clone())
    }
}

#[async_trait]
impl ChunkReader for DsgStackChunkReader {
    fn clone_as_reader(&self) -> Box<dyn ChunkReader> {
        Box::new(self.clone())
    }

    async fn exists(&self, chunk: &ChunkId) -> bool {
        self.0.ndn_service().get_data(NDNGetDataOutputRequest::new(NDNAPILevel::NDC, chunk.object_id(), None)).await.is_ok()
    }

    async fn get(&self, chunk: &ChunkId) -> BuckyResult<Arc<Vec<u8>>> {
        let mut resp = self.0.ndn_service().get_data(NDNGetDataOutputRequest::new(NDNAPILevel::NDC, chunk.object_id(), None)).await?;
        let mut data = vec![0u8; chunk.len() as usize];
        let mut buf = data.as_mut_slice();
        loop {
            let len = resp.data.read(buf).await?;
            buf = &mut buf[len..];
            if len == 0 || buf.len() == 0 {
                break;
            }
        }
        Ok(Arc::new(data))
    }
}


#[derive(Clone, Debug)]
pub struct DsgChunkMergeStub {
    pub first_range: Option<u32>, 
    pub indices: Vec<u32>, 
    pub last_range: Option<u32> 
}

impl DsgChunkMergeStub {
    pub fn first_range(len: u32) -> Self {
        Self {
            first_range: Some(len), 
            indices: vec![], 
            last_range: None
        }
    }

    pub fn first_chunk(len: u32) -> Self {
        Self {
            first_range: None,
            indices: vec![len], 
            last_range: None
        }
    }

    pub fn chunk_count(&self) -> usize {
        let mut count = 0;
        count += if self.first_range.is_some() {
            1 
        } else {
            0
        };
        count += self.indices.len();
        count += if self.last_range.is_some() {
            1 
        } else {
            0
        };
        count
    }

    pub fn to_ranges(&self, first_offset: usize) -> LinkedList<Range<usize>> {
        let mut ranges = LinkedList::new();
        if let Some(len) = &self.first_range {
            ranges.push_back(first_offset..first_offset + *len as usize);
        } 
        for len in &self.indices {
            ranges.push_back(0..*len as usize);
        }
        if let Some(len) = &self.last_range {
            ranges.push_back(0..*len as usize);
        } 
        ranges
    }

    pub fn pop_front(&mut self) -> Option<(usize, bool)> {
        if let Some(len) = self.first_range {
            self.first_range = None;
            Some((len as usize, true))
        } else if self.indices.len() > 0 {
            Some((self.indices.remove(0) as usize, false))
        } else if let Some(len) = self.last_range {
            self.last_range = None;
            Some((len as usize, true))
        } else {
            None
        }
    }
}

impl TryFrom<&DsgChunkMergeStub> for protos::ChunkMergeStub {
    type Error = BuckyError;

    fn try_from(rust: &DsgChunkMergeStub) -> BuckyResult<Self> {
        let mut proto = protos::ChunkMergeStub::new();

        if let Some(len) = rust.first_range.as_ref() {
            proto.set_first_range(*len);
        }
      
        proto.set_index_range(rust.indices.clone());

        if let Some(len) = rust.last_range.as_ref() {
            proto.set_last_range(*len);

        }

        Ok(proto)
    }
}

impl TryFrom<protos::ChunkMergeStub> for DsgChunkMergeStub {
    type Error = BuckyError;

    fn try_from(mut proto: protos::ChunkMergeStub) -> BuckyResult<Self> {
        let stub = Self {
            first_range: if proto.has_first_range() {
                Some(proto.get_first_range())
            } else {
                None
            }, 
            indices: proto.take_index_range(), 
            last_range: if proto.has_last_range() {
                Some(proto.get_last_range())
            } else {
                None
            }
        };
        Ok(stub)

    }
}

impl_default_protobuf_raw_codec!(DsgChunkMergeStub, protos::ChunkMergeStub);

#[derive(Clone, Debug)]
pub struct DsgChunkFunctionMerge {
    pub key: Option<AesKey>, 
    pub chunks: DsgChunkMergeStub, 
    pub split: u32

}

impl TryFrom<&DsgChunkFunctionMerge> for protos::ChunkFunctionMerge {
    type Error = BuckyError;

    fn try_from(rust: &DsgChunkFunctionMerge) -> BuckyResult<Self> {
        let mut proto = protos::ChunkFunctionMerge::new();
        if let Some(key) = &rust.key {
            proto.set_key(key.to_vec()?);
        }
        proto.set_chunks(protos::ChunkMergeStub::try_from(&rust.chunks)?);
        proto.set_split(rust.split);
        Ok(proto)
    }
}

impl TryFrom<protos::ChunkFunctionMerge> for DsgChunkFunctionMerge {
    type Error = BuckyError;

    fn try_from(mut proto: protos::ChunkFunctionMerge) -> BuckyResult<Self> {
        Ok(Self {
            key: if proto.has_key() {
                Some(ProtobufCodecHelper::decode_buf(proto.take_key())?)
            } else {
                None
            }, 
            chunks: DsgChunkMergeStub::try_from(proto.take_chunks())?, 
            split: proto.get_split()
        })
    }
}

impl_default_protobuf_raw_codec!(DsgChunkFunctionMerge, protos::ChunkFunctionMerge);

#[derive(Clone)]
pub struct DsgDataSourceStubDesc {
    pub functions: Vec<DsgChunkFunctionMerge>
}

impl TryFrom<&DsgDataSourceStubDesc> for protos::DataSourceStubDesc {
    type Error = BuckyError;

    fn try_from(rust: &DsgDataSourceStubDesc) -> BuckyResult<Self> {
        let mut proto = protos::DataSourceStubDesc::new();
        proto.set_functions(ProtobufCodecHelper::encode_nested_list(&rust.functions)?);
        Ok(proto)
    }
}

impl TryFrom<protos::DataSourceStubDesc> for DsgDataSourceStubDesc {
    type Error = BuckyError;

    fn try_from(mut proto: protos::DataSourceStubDesc) -> BuckyResult<Self> {
        Ok(Self {
            functions: ProtobufCodecHelper::decode_nested_list(proto.take_functions())?
        })
    }
}

impl_default_protobuf_raw_codec!(DsgDataSourceStubDesc, protos::DataSourceStubDesc);

impl DescContent for DsgDataSourceStubDesc {
    fn obj_type() -> u16 {
        obj_id::CONTRACT_DATA_SOURCE_STUB_OBJECT_TYPE
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = SubDescNone;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(RawEncode, RawDecode, Clone)]
pub struct DsgDataSourceStubBody {}

impl BodyContent for DsgDataSourceStubBody {}

pub type DsgDataSourceStubObjectType = NamedObjType<DsgDataSourceStubDesc, DsgDataSourceStubBody>;
pub type DsgDataSourceStubObject = NamedObjectBase<DsgDataSourceStubObjectType>;

#[derive(Copy, Clone)]
pub struct DsgDataSourceStubObjectRef<'a> {
    obj: &'a DsgDataSourceStubObject,
}

impl<'a> std::fmt::Display for DsgDataSourceStubObjectRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DsgDataSourceStubObjectRef{{id={}, functions={:?}}}",
            self.id(),
            self.functions()
        )
    }
}

impl<'a> AsRef<DsgDataSourceStubObject> for DsgDataSourceStubObjectRef<'a> {
    fn as_ref(&self) -> &DsgDataSourceStubObject {
        self.obj
    }
}

impl<'a> From<&'a DsgDataSourceStubObject> for DsgDataSourceStubObjectRef<'a> {
    fn from(obj: &'a DsgDataSourceStubObject) -> Self {
        Self { obj }
    }
}




#[derive(Clone)]
struct SourceReaderIter {
    range: Range<usize>, 
    index: usize, 
    cache: Option<Arc<Vec<u8>>>
}

impl SourceReaderIter {
    async fn cache(&mut self, reader: &Box<dyn ChunkReader>, sources: &ChunkListDesc) -> BuckyResult<Arc<Vec<u8>>> {
        if let Some(cache) = self.cache.as_ref() {
            return Ok(cache.clone());
        } 
        let cache = reader.get(&sources.chunks()[self.index as usize]).await?;
        self.cache = Some(cache);
        Ok(self.cache.clone().unwrap())
    }

    fn next(&self, stub: &DsgChunkMergeStub) -> Self {
        let mut _next = Self {
            range: 0..0,
            index: 0, 
            cache: None
        };
        if let Some(range) = &stub.first_range {
            _next.range = self.range.end..self.range.end + *range as usize;
            _next.index = self.index;
            _next.cache = self.cache.clone();
        } else {
            _next.range = 0..stub.indices[0] as usize;
            _next.index = self.index + 1;
        }
        _next
    }
 
    fn start(stub: &DsgChunkMergeStub) -> Self {
        Self {
            index: 0, 
            range: if let Some(len) = &stub.first_range {
                0..*len as usize
            } else {
                0..stub.indices[0] as usize
            }, 
            cache: None
        }
    }
}

#[derive(Clone)]
enum MergeWriterState {
    Header, 
    Chunks
}


struct MergeWriter {
    state: MergeWriterState, 
    source: SourceReader,  
    function: DsgChunkFunctionMerge
}


impl std::fmt::Display for MergeWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MergeWriter{{function={:?}}}",
            self.function
        )
    }
}

impl MergeWriter {
    fn new(source: SourceReader, function: DsgChunkFunctionMerge) -> Self {
        Self {
            state: MergeWriterState::Header, 
            source, 
            function
        }
    }

    #[async_recursion]
    async fn skip_block_inner(&mut self, block_len: usize) -> BuckyResult<usize> {
        match self.state.clone() {
            MergeWriterState::Header => unreachable!(),
            MergeWriterState::Chunks => {
                let enc_block = DsgDataSourceStubObjectRef::enc_block_len();
                let mut total = 0;
                for _ in 0..(block_len / enc_block) {
                    let read = self.source.next(None).await?;
                    total += read;
                    if read < enc_block {
                        break;
                    }
                }

                if total == 0 {
                    return Ok(0);
                }
                
                if total % enc_block != 0 {
                    let padding_len = enc_block - total % enc_block;
                    Ok(total + padding_len)
                } else {
                    Ok(total)
                } 
            }
        }
    }

    async fn skip_block(&mut self, stack: &SharedCyfsStack) -> BuckyResult<usize> {
        match self.state.clone() {
            MergeWriterState::Header => {
                let header_len = MergeMeasure::header(stack, &self.function);
                self.skip_block_inner(self.function.split as usize - header_len).await
            }, 
            MergeWriterState::Chunks => {
                self.skip_block_inner(self.function.split as usize).await
            }
        }
    }

    #[async_recursion]
    async fn next_block_inner(&mut self, block: &mut [u8]) -> BuckyResult<usize> {
        let enc_block = DsgDataSourceStubObjectRef::enc_block_len();
        let block_len = block.len();
        match self.state.clone() {
            MergeWriterState::Header => unreachable!(),
            MergeWriterState::Chunks => {
                let mut total = 0;
                let mut ptr = &mut block[..]; 
                for _ in 0..(block_len / enc_block) {
                    let read = self.source.next(Some(ptr)).await?;
                    total += read;
                    if read < enc_block {
                        break;
                    }
                    ptr = &mut ptr[enc_block..];
                }

                log::debug!("{} read source, len={}", self, total);

                if total == 0 {
                    return Ok(0);
                }

                
                total = if total % enc_block != 0 {
                    let padding_len = enc_block - total % enc_block;
                    let padding = vec![0u8; padding_len];
                    block[total..total + padding_len].copy_from_slice(padding.as_slice());
                    total + padding_len
                } else {
                    total
                };

                log::debug!("{} pad source, len={}", self, total);
                
                if let Some(key) = self.function.key.as_ref() {
                    let enc_total = DsgDataSourceStubObjectRef::enc_block(key, block, total)?;
                    assert_eq!(enc_total, total);
                } 

                Ok(total)
            }
        }
    }

    async fn next_block(&mut self, stack: &SharedCyfsStack, block: &mut [u8]) -> BuckyResult<usize> {
        let enc_block = DsgDataSourceStubObjectRef::enc_block_len();
        match self.state.clone() {
            MergeWriterState::Header => {
                let mut header_len = u8::raw_bytes().unwrap();

                let ptr = 0u8.raw_encode(block, &None)?;
                if let Some(key) = self.function.key.as_ref() {
                    let key_len = AesKey::default().raw_measure(&None).unwrap();
                    let mut key_buf = vec![0u8; key_len];
                    let _ = key.raw_encode(key_buf.as_mut_slice(), &None)?;
                    let encrypt_len = stack.local_device().desc().public_key().encrypt(&key_buf.as_slice(), ptr)?;  
                    header_len += encrypt_len;
                } 

                let mut ranges = self.function.chunks.to_ranges(self.source.start.range.start);

                let mut ptr = (ranges.len() as u8).raw_encode(&mut block[header_len..], &None)?;
                header_len += u8::raw_bytes().unwrap();

                let range_len = Range::<u32>::raw_bytes().unwrap();
                
                loop {
                    if let Some(range) = ranges.pop_front() {
                        let range = range.start as u32..range.end as u32;
                        ptr = range.raw_encode(ptr, &None)?;
                        header_len += range_len;
                    } else {
                        break;
                    }
                }
                
                assert!(header_len < block.len());

                log::debug!("{} write header, len={}", self, header_len);

                header_len = if header_len % enc_block == 0 {
                    header_len
                } else {
                    let padded = ((header_len / enc_block) + 1) * enc_block;
                    for i in header_len..padded  {
                        block[i] = 0u8;
                    }
                    padded
                };

                log::debug!("{} pad header, len={}", self, header_len);
                self.state = MergeWriterState::Chunks;

                self.next_block_inner(&mut block[header_len..]).await
            }, 
            MergeWriterState::Chunks => {
                self.next_block_inner(block).await
            }
        }
    }
}

struct MergeMeasure {
    cur: Option<SourceReaderIter>, 
    sources: ChunkListDesc
}

impl MergeMeasure {
    fn header(stack: &SharedCyfsStack, function: &DsgChunkFunctionMerge) -> usize {
        let mut header_len = u8::raw_bytes().unwrap();

        if function.key.is_some() {
            let encrypt_len = stack.local_device().desc().public_key().key_size();
            header_len += encrypt_len;
        } 

        header_len += Vec::<u8>::raw_min_bytes().unwrap();

        let range_len = Range::<u32>::raw_bytes().unwrap();
        header_len += function.chunks.chunk_count() * range_len;

        let enc_block = DsgDataSourceStubObjectRef::enc_block_len();
        header_len = if header_len % enc_block == 0 {
            header_len
        } else {
            ((header_len / enc_block) + 1) * enc_block
        };
       
        assert_eq!(header_len % enc_block, 0);

        header_len
    } 

    fn new(sources: ChunkListDesc) -> Self {
        Self {
            cur: Some(SourceReaderIter {
                range: 0..0, 
                index: 0, 
                cache: None
            }), 
            sources
        }
    }

    fn next(&mut self, stack: &SharedCyfsStack, key: &Option<AesKey>) -> Option<DsgChunkMergeStub> {
        if let Some(cur) = self.cur.clone() {
            let mut header_len = u8::raw_bytes().unwrap();
           
            if key.is_some() {
                let encrypt_len = stack.local_device().desc().public_key().key_size();
                header_len += encrypt_len;
            } 
            
            header_len += Vec::<u8>::raw_min_bytes().unwrap();

            let range_len = Range::<u32>::raw_bytes().unwrap();
    
            let mut source_iter = cur;
            let mut stub = DsgChunkMergeStub {
                first_range: None, 
                indices: vec![], 
                last_range: None 
            };

            let enc_block = DsgDataSourceStubObjectRef::enc_block_len();

            let cur = loop {
                let mut next_header_len = header_len as u64 + range_len as u64;
                next_header_len = if next_header_len % enc_block as u64 == 0 {
                    next_header_len
                } else {
                    ((next_header_len / enc_block as u64) + 1) * enc_block as u64
                };
                  
                if next_header_len > u32::MAX as u64 {
                    break Some(source_iter);
                } 
                let remain_chunk_len = u32::MAX as usize - next_header_len as usize;
                let cur_id = &self.sources.chunks()[source_iter.index];

                header_len += range_len;
                if (source_iter.range.end + remain_chunk_len) <= cur_id.len() {
                    if stub.indices.len() > 0 {
                        stub.last_range = Some(remain_chunk_len as u32);
                    } else {
                        stub.first_range = Some(remain_chunk_len as u32);
                    }
                    source_iter.range.end += remain_chunk_len;
                    break Some(source_iter);
                } else {
                    if source_iter.range.end == 0 {
                        stub.indices.push(cur_id.len() as u32);
                    } else {
                        if stub.indices.len() > 0 {
                            stub.last_range = Some((cur_id.len() - source_iter.range.end) as u32);
                        } else {
                            stub.first_range = Some((cur_id.len() - source_iter.range.end) as u32);
                        }
                    }
                    source_iter.index += 1;
                    if source_iter.index >= self.sources.chunks().len() {
                        break None;
                    } else {
                        source_iter.range = 0..0;
                        break Some(source_iter);
                    }
                }
            };
            self.cur = cur;
            Some(stub)
        } else {
            None
        }
       
    }
}


struct SourceReader {
    reader: Box<dyn ChunkReader>, 
    stub: DsgChunkMergeStub, 
    block: usize,  
    cur: Option<SourceReaderIter>, 
    start: SourceReaderIter,  
    sources: ChunkListDesc
}

impl Clone for SourceReader {
    fn clone(&self) -> Self {
        Self {
            reader: self.reader.clone_as_reader(), 
            stub: self.stub.clone(), 
            block: self.block, 
            cur: self.cur.clone(), 
            start: self.start.clone(), 
            sources: self.sources.clone()
        }
    }
}

impl SourceReader {
    fn next_reader(
        self, 
        stub: DsgChunkMergeStub, 
        block: usize
    ) -> Self {
        let start = self.start.next(&stub);
        Self {
            reader: self.reader, 
            block,  
            cur: Some(start.clone()), 
            start, 
            stub, 
            sources: self.sources 
        }
    }


    fn first_reader(
        reader: Box<dyn ChunkReader>, 
        sources: ChunkListDesc, 
        stub: DsgChunkMergeStub, 
        block: usize
    ) -> Self {
        let start = SourceReaderIter::start(&stub);
        Self {
            reader, 
            block, 
            cur: Some(start.clone()), 
            start, 
            stub, 
            sources, 
        }
    }

   
    async fn skip(&mut self, block_count: usize) -> BuckyResult<u64> {
        let mut skip = 0u64;
        for _ in 0..block_count {
            let block = self.next(None).await?;
            skip += block as u64;
            if block < self.block {
                break;
            }
        }
        Ok(skip)
    }

    async fn next(&mut self, buf: Option<&mut [u8]>) -> BuckyResult<usize> {
        if let Some(cur) = &mut self.cur {
            let cur_remain = cur.range.end - cur.range.start;
            let (read, cur) = if cur_remain > self.block {
                if buf.is_some() {
                    buf.unwrap()[..self.block].copy_from_slice(&cur.cache(&self.reader, &self.sources).await?[cur.range.start..cur.range.start + self.block]);
                }
                let mut cur = cur.clone();
                cur.range.start += self.block;
                (self.block, Some(cur))
            } else {
                let mut offset = 0;
                let mut cur_remain = cur_remain;
                let mut cur = cur.clone();
                let copy = buf.is_some();
                let mut placehoder = [0u8; 0];
                let dst_buf = if copy {
                    buf.unwrap()
                } else {
                    &mut placehoder
                };
                let cur = loop {
                    if copy {
                        dst_buf[offset..offset + cur_remain].copy_from_slice(&cur.cache(&self.reader, &self.sources).await?[cur.range.start..cur.range.start + cur_remain]);
                    }
                    offset += cur_remain;
                    cur.range.start += cur_remain;
                    if cur.range.start >= cur.range.end {
                        cur.index += 1;
                        if cur.index == self.start.index + self.stub.chunk_count() {
                            break None;
                        } else {
                            let cur_id = &self.sources.chunks()[cur.index as usize];
                            if cur.index == (self.start.index + self.stub.chunk_count() - 1) {
                                cur.range = if let Some(len) = &self.stub.last_range {
                                    0..*len as usize
                                } else {
                                    0..cur_id.len() as usize
                                };
                            } else {
                                let cur_id = &self.sources.chunks()[cur.index as usize];
                                cur.range = 0..cur_id.len() as usize;
                            }
                            
                            if offset >= self.block {
                                break Some(cur);
                            }

                            cur_remain = self.block - offset;
                            let cur_len = cur.range.end - cur.range.start;
                            cur_remain = if cur_len > cur_remain {
                                cur_remain
                            } else {
                                cur_len
                            };
                            continue;
                        }
                    } else {
                        break Some(cur);
                    }
                };
                (offset, cur)
            };
            self.cur = cur;
            Ok(read)
        } else {
            Ok(0)
        }
    } 
}


enum RestoreReaderState {
    Header, 
    Ranges, 
    Chunks
}

struct RestoreReaderIter {
    state: RestoreReaderState, 
    function: DsgChunkFunctionMerge, 
    func_index: usize, 
    split_index: usize, 
    split_offset: usize, 
    split_cache: Option<Arc<Vec<u8>>>
}

impl RestoreReaderIter {
    fn start(stack: &SharedCyfsStack, functions: Option<&Vec<DsgChunkFunctionMerge>>) -> Self {
        if let Some(functions) = functions {
            let function = functions[0].clone();
            let header_len = MergeMeasure::header(stack, &function);
            let split_index = header_len / function.split as usize;
            let split_offset = header_len % function.split as usize;
            Self {
                state: RestoreReaderState::Chunks, 
                function, 
                func_index: 0, 
                split_index,  
                split_offset,  
                split_cache: None
            }
        } else {
            unimplemented!()
        }
    }

    fn next_func(&mut self, stack: &SharedCyfsStack, functions: Option<&Vec<DsgChunkFunctionMerge>>) -> bool {
        if let Some(functions) = functions {
            let func_index = self.func_index + 1;
            if func_index >= functions.len() {
                false
            } else {
                let function = functions[func_index].clone();
                let header_len = MergeMeasure::header(stack, &function);
                let split_index = header_len / function.split as usize;
                let split_offset = header_len % function.split as usize;
                
                self.state = RestoreReaderState::Chunks; 
                self.function = function; 
                self.func_index = func_index;
                self.split_index += 1 + split_index;
                self.split_offset = split_offset;
                true
            }
        } else {
            unimplemented!()
        }
    }

    async fn read_from_cache(
        &mut self, 
        merged: &ChunkListDesc, 
        reader: Box<dyn ChunkReader>, 
        buffer: &mut [u8]
    ) -> BuckyResult<usize> {
        let _ = self.load_chunk_split(merged, reader).await?;
        if let Some(cache) = self.split_cache.clone() { 
            let read = if self.split_offset + buffer.len() > cache.len() {
                cache.len() - self.split_offset
            } else {
                buffer.len()
            };
            if read > 0 {
                buffer.copy_from_slice(&cache[self.split_offset..self.split_offset + read]);
                self.split_offset += read;
            }
            Ok(read)
        } else {
            Ok(0)
        }
    }

    fn next_split(&mut self, merged: &ChunkListDesc) -> bool {
        let split_index = self.split_index + 1;
        if split_index >= merged.chunks().len() {
           false
        } else {
            self.split_index = split_index;
            self.split_offset = 0;
            self.split_cache = None;
            true
        } 
    }

    async fn load_chunk_split(
        &mut self, 
        merged: &ChunkListDesc, 
        reader: Box<dyn ChunkReader>
    ) -> BuckyResult<usize> {
        if self.split_index >= merged.chunks().len() {
            Ok(0)
        } else {
            if let Some(cache) = self.split_cache.clone() {
                Ok(cache.len())
            } else {
                let chunk = &merged.chunks()[self.split_index];
                let data = reader.get(chunk).await?;
                match &self.state {
                    RestoreReaderState::Header => unreachable!(), 
                    RestoreReaderState::Ranges => unreachable!(), 
                    RestoreReaderState::Chunks => {
                        let data = if let Some(key) = self.function.key.as_ref() {
                            let mut buffer = vec![0u8; data.len()];
                            let dec_len = DsgDataSourceStubObjectRef::dec_block(key, &mut buffer[self.split_offset..], data.len() - self.split_offset)?;
                            Arc::new(buffer)
                        } else {
                            data
                        };
                        self.split_cache = Some(data.clone());
                        Ok(data.len())
                    }, 
                }
            }
           
        }
    }
}

struct RestoreReader {
    stack: Arc<SharedCyfsStack>, 
    reader: Box<dyn ChunkReader>, 
    merged: ChunkListDesc, 
    merged_iter: Option<RestoreReaderIter>, 
    functions: Option<Vec<DsgChunkFunctionMerge>>,  
    next_cache: Option<Vec<u8>>
}


impl RestoreReader {
    fn new(
        stack: Arc<SharedCyfsStack>, 
        reader: Box<dyn ChunkReader>, 
        merged: ChunkListDesc, 
        functions: Option<Vec<DsgChunkFunctionMerge>>, 
    ) -> Self {
        let merged_iter = RestoreReaderIter::start(stack.as_ref(), functions.as_ref());
        Self {
            stack, 
            functions, 
            merged, 
            reader, 
            merged_iter: Some(merged_iter), 
            next_cache: None
        }
    }

    async fn next(&mut self) -> BuckyResult<Option<Vec<u8>>> {
        if self.next_cache.is_some() {
           let mut next = None;
           std::mem::swap(&mut self.next_cache, &mut next);
           Ok(next)  
        } else {
            let mut ranges = LinkedList::new();
            loop {
                if let Some((data, ranged)) = self.next_inner().await? {
                    if !ranged {
                        if ranges.len() > 0 {
                            self.next_cache = Some(data);
                        } else {
                            ranges.push_back(data);
                        }
                        break;
                    } else {
                        ranges.push_back(data);
                    }
                } else {
                    break;
                }
            }
            if ranges.len() > 1 {
                let total = ranges.iter().fold(0, |l, d| l + d.len());
                let mut data = vec![0u8; total];
                let mut offset = 0;
                for r in ranges {
                    data[offset..offset + r.len()].copy_from_slice(&r[..]);
                    offset += r.len();
                } 
                Ok(Some(data))
            } else {
                Ok(ranges.pop_front())
            }
        }
        
    }

    #[async_recursion]
    async fn next_inner(&mut self) -> BuckyResult<Option<(Vec<u8>, bool)>> {
        if let Some(iter) = &mut self.merged_iter {
            match &iter.state {
                RestoreReaderState::Header => unimplemented!(), 
                RestoreReaderState::Ranges => unimplemented!(), 
                RestoreReaderState::Chunks => {
                    if let Some((len, ranged)) = iter.function.chunks.pop_front() {
                        let mut buffer = vec![0u8; len];
                        let mut offset = 0;
                        loop {
                            let read = iter.read_from_cache(&self.merged, self.reader.clone_as_reader(), &mut buffer[offset..]).await?;
                            if read < len - offset {
                                if !iter.next_split(&self.merged) {
                                    unreachable!()   
                                }
                                offset += read;
                            } else {
                                break;
                            }
                        }
                        Ok(Some((buffer, ranged)))
                    } else {
                        if iter.next_func(self.stack.as_ref(), self.functions.as_ref()) {
                            self.merged_iter = None;
                            Ok(None)
                        } else {
                            self.next_inner().await
                        }
                    }
                }
            }
        } else {
            Ok(None)
        }
    }
}



impl<'a> DsgDataSourceStubObjectRef<'a> {
    pub fn functions(&self) -> &Vec<DsgChunkFunctionMerge> {
        &self.obj.desc().content().functions
    }

    pub fn id(&self) -> ObjectId {
        self.obj.desc().object_id()
    }

    pub fn unchanged() -> DsgDataSourceStubObject {
        NamedObjectBuilder::new(
            DsgDataSourceStubDesc {
                functions: vec![]
            }, 
            DsgDataSourceStubBody {})
        .no_create_time().build()
    }

    pub fn is_unchanged(&self) -> bool {
        self.obj.desc().content().functions.len() == 0
    }


    pub fn enc_block_len() -> usize {
        <Aes256 as BlockCipher>::BlockSize::to_usize()
    }


    pub fn enc_block(key: &AesKey, buffer: &mut [u8], len: usize) -> BuckyResult<usize> {
        assert_eq!(buffer.len() % Self::enc_block_len(), 0);
        let key = key.as_ref().as_slice();
        let cipher = Cbc::<Aes256, NoPadding>::new_from_slices(&key[0..32], &key[32..]).unwrap();

        match cipher.encrypt(buffer, len) {
            Ok(buf) => Ok(buf.len()),
            Err(e) => {
                let msg = format!(
                    "AesKey::inplace_encrypt error, inout={}, in_len={}, {}",
                    buffer.len(),
                    len,
                    e
                );
                Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg))
            }
        }
    }

    pub fn dec_block(key: &AesKey, buffer: &mut [u8], len: usize) -> BuckyResult<usize> {
        let key = key.as_ref().as_slice();
        let cipher = Cbc::<Aes256, NoPadding>::new_from_slices(&key[0..32], &key[32..]).unwrap();
        match cipher.decrypt(&mut buffer[..len]) {
            Ok(buf) => Ok(buf.len()),
            Err(e) => {
                let msg = format!(
                    "AesKey::inplace_decrypt error, inout={}, in_len={}, {}",
                    buffer.len(),
                    len,
                    e
                );
                Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg))
            }
        }
    }

    pub fn merge_with_key(
        stack: &SharedCyfsStack, 
        sources: ChunkListDesc, 
        aes_key: AesKey, 
        split: u32
    ) -> DsgDataSourceStubObject {
        let mut measure = MergeMeasure::new(sources);
        let key = Some(aes_key);
        let mut functions = vec![];
        loop {
            if let Some(stub) = measure.next(stack, &key) {
                functions.push(DsgChunkFunctionMerge { key: key.clone(), chunks: stub, split });
            } else {
                break;
            }
        }

        NamedObjectBuilder::new(
            DsgDataSourceStubDesc { functions }, 
            DsgDataSourceStubBody {})
        .no_create_time().build()
    }



    pub async fn apply(
        &self, 
        stack: Arc<SharedCyfsStack>, 
        sources: ChunkListDesc, 
    ) -> BuckyResult<Vec<ChunkId>> {
        if self.is_unchanged() {
            let reader = DsgStackChunkReader::new(stack.clone()).clone_as_reader();
            for chunk in sources.chunks() {
                let chunk_data = reader.get(chunk).await?;
                let mut hasher = Sha256::new();
                hasher.input(chunk_data.as_ref());
                let verify_chunk = ChunkId::new(&hasher.result().into(), chunk_data.len() as u32);
                let _ = if !verify_chunk.eq(chunk) {
                    Err(BuckyError::new(BuckyErrorCode::InvalidData, "source chunk invalid"))
                } else {
                    Ok(())
                }?;
            }
            Ok(Vec::from(sources.chunks()))
        } else {

            async fn add_chunk(stack: &SharedCyfsStack, buffer: &[u8]) -> BuckyResult<ChunkId> {
                let mut hasher = Sha256::new();
                hasher.input(buffer);
                let chunk_id = ChunkId::new(&hasher.result().into(), buffer.len() as u32);
                let _ = stack.ndn_service().put_data(NDNPutDataOutputRequest::new(NDNAPILevel::NDC, chunk_id.object_id(), buffer.len() as u64, Box::new(Cursor::new(Vec::from(buffer))))).await?;
                Ok(chunk_id)
            } 

            let mut chunks = vec![];
            let mut pre_reader: Option<SourceReader> = None;

            for f in self.functions() {
                let mut buffer = vec![0u8; f.split as usize];

                let enc_block = Self::enc_block_len();

                let mut _pre_reader = None;
                std::mem::swap(&mut _pre_reader, &mut pre_reader);

                let reader = if let Some(pre_reader) = _pre_reader {
                    pre_reader.next_reader(f.chunks.clone(), enc_block)
                } else {
                    SourceReader::first_reader(DsgStackChunkReader::new(stack.clone()).clone_as_reader(), sources.clone(), f.chunks.clone(), enc_block)
                };
                let mut writer = MergeWriter::new(reader.clone(), f.clone());

                loop {
                    let written = writer.next_block(stack.as_ref(), &mut buffer[..]).await?;
                    if written == 0 {
                        break;
                    }
                    chunks.push(add_chunk(stack.as_ref(), &buffer[..written]).await?);
                }
               
                pre_reader = Some(reader);
            }

            Ok(chunks)
        }
    }

    pub async fn restore(
        &self, 
        stack: Arc<SharedCyfsStack>, 
        merged: ChunkListDesc, 
        reader: Box<dyn ChunkReader>
    ) -> BuckyResult<Vec<ChunkId>> {
        if self.is_unchanged() {
            Ok(Vec::from(merged.chunks()))
        } else {
            let mut reader = RestoreReader::new(
                stack.clone(), 
                reader, 
                merged, 
                Some(self.functions().clone()));
            let mut sources = vec![];
            loop {
                if let Some(data) = reader.next().await? {
                    let mut hasher = Sha256::new();
                    hasher.input(&data[..]);
                    let chunk = ChunkId::new(&hasher.result().into(), data.len() as u32);
                 
                    let _ = stack.ndn_service().put_data(NDNPutDataOutputRequest::new(
                        NDNAPILevel::NDC,
                        chunk.object_id().clone(),
                        chunk.len() as u64,
                        Box::new(Cursor::new(data)))).await?;

                    sources.push(chunk);    
                } else {
                    break;
                }
            }
            Ok(sources)
        }
    }

    pub async fn read_sample(
        &self,  
        stack: &SharedCyfsStack, 
        reader: &Box<dyn ChunkReader>, 
        merged: ChunkListDesc, 
        sources: ChunkListDesc,
        sample: &DsgChallengeSample,
    ) -> BuckyResult<Box<dyn Read + Unpin + Send + Sync>> {
        if self.is_unchanged() {
            reader.read_ext(
                &sources.chunks()[sample.chunk_index as usize], 
                vec![sample.offset_in_chunk..(sample.offset_in_chunk + sample.sample_len as u64)]
            ).await
        } else {
            let mut pre_reader: Option<SourceReader> = None;
            let mut block_index = 0;
            let mut func_iter = self.functions().iter();
            let enc_block = Self::enc_block_len();
            let ret_writer = loop {
                if let Some(func) = func_iter.next() {
                    let mut _pre_reader = None;
                    std::mem::swap(&mut _pre_reader, &mut pre_reader);

                    let reader = if let Some(pre_reader) = _pre_reader {
                        pre_reader.next_reader(func.chunks.clone(), enc_block)
                    } else {
                        SourceReader::first_reader(reader.clone_as_reader(), sources.clone(), func.chunks.clone(), enc_block)
                    };
                    let mut writer = MergeWriter::new(reader.clone(), func.clone());

                    let ret_writer = loop {
                        if block_index == sample.chunk_index {
                            break Some(writer);
                        }
                        let written = writer.skip_block(stack).await?;
                        if written == 0 {
                            break None;
                        }
                        block_index += 1;
                        
                    };

                    if ret_writer.is_some() {
                        break ret_writer;
                    }

                    pre_reader = Some(reader);
                } else {
                    break None;
                }
            };

            if let Some(mut writer) = ret_writer {
                let mut buffer = vec![0u8; writer.function.split as usize];
                let total = writer.next_block(stack, &mut buffer[..]).await?;
                let range = sample.offset_in_chunk as usize..sample.offset_in_chunk as usize + sample.sample_len as usize;
                if total < range.end  {
                    Err(BuckyError::new(BuckyErrorCode::OutOfLimit, ""))
                } else {
                    let result = Vec::from(&buffer[range]);
                    Ok(Box::new(Cursor::new(result)))
                }
            } else {
                Err(BuckyError::new(BuckyErrorCode::OutOfLimit, ""))
            }
        }
    }
}
