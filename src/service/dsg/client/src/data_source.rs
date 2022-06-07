use std::{
    sync::Arc, 
    convert::TryFrom, 
    ops::Range
};
use async_std::{
    io::{prelude::*, Cursor}, 
};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
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
        resp.data.read(data.as_mut_slice()).await?;
        Ok(Arc::new(data))
    }
}


#[derive(Clone, Debug)]
pub struct DsgChunkMergeStub {
    pub offset: u64, 
    pub len: u64, 
    pub first_range: Option<Range<u64>>, 
    pub indices: Range<u32>, 
    pub last_range: Option<Range<u64>> 
}

impl DsgChunkMergeStub {
    pub fn first(len: u64, index: usize, range: Option<Range<u64>>) -> Self {
        Self {
            offset: 0, 
            len,
            first_range: range, 
            indices: index as u32..index as u32 + 1, 
            last_range: None
        }
    }
}

impl TryFrom<&DsgChunkMergeStub> for protos::ChunkMergeStub {
    type Error = BuckyError;

    fn try_from(rust: &DsgChunkMergeStub) -> BuckyResult<Self> {
        let mut proto = protos::ChunkMergeStub::new();

        proto.set_len(rust.len);
        proto.set_offset(rust.offset);

        if let Some(range) = rust.first_range.as_ref() {
            proto.set_first_range_start(range.start);
            proto.set_first_range_end(range.end);
        }
      
        proto.set_index_range_start(rust.indices.start);
        proto.set_index_range_end(rust.indices.end);

        if let Some(range) = rust.last_range.as_ref() {
            proto.set_last_range_start(range.start);
            proto.set_last_range_end(range.end);

        }

        Ok(proto)
    }
}

impl TryFrom<protos::ChunkMergeStub> for DsgChunkMergeStub {
    type Error = BuckyError;

    fn try_from(proto: protos::ChunkMergeStub) -> BuckyResult<Self> {

        let stub = Self {
            len: proto.get_len(), 
            offset: proto.get_offset(), 
            first_range: if proto.has_first_range_start() {
                Some(proto.get_first_range_start()..proto.get_first_range_end())
            } else {
                None
            }, 
            indices: proto.get_index_range_start()..proto.get_index_range_end(), 
            last_range: if proto.has_last_range_start() {
                Some(proto.get_last_range_start()..proto.get_last_range_end())
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
struct CurChunk {
    range: Range<usize>, 
    index: u32, 
    cache: Option<Arc<Vec<u8>>>
}

impl CurChunk {
    async fn cache(&mut self, reader: &Box<dyn ChunkReader>, sources: &ChunkListDesc) -> BuckyResult<Arc<Vec<u8>>> {
        if let Some(cache) = self.cache.as_ref() {
            return Ok(cache.clone());
        } 
        let cache = reader.get(&sources.chunks()[self.index as usize]).await?;
        self.cache = Some(cache);
        Ok(self.cache.clone().unwrap())
    }
}

struct MergeReader {
    reader: Box<dyn ChunkReader>, 
    stub: DsgChunkMergeStub, 
    block: usize,  
    cur: Option<CurChunk>
}

impl MergeReader {
    async fn new(reader: Box<dyn ChunkReader>, sources: &ChunkListDesc, stub: DsgChunkMergeStub, block: usize) -> BuckyResult<Self> {
        let cur_id = &sources.chunks()[stub.indices.start as usize];
        Ok(Self {
            block, 
            cur: Some(CurChunk {
                index: stub.indices.start, 
                range: if let Some(range) = &stub.first_range {
                    range.start as usize..range.end as usize
                } else {
                    0..cur_id.len() as usize
                }, 
                cache: None
            }),
            stub, 
            reader, 
        })
    }

    async fn skip(&mut self, sources: &ChunkListDesc, block_count: usize) -> BuckyResult<u64> {
        let mut skip = 0u64;
        for _ in 0..block_count {
            let block = self.next(sources, None).await?;
            skip += block as u64;
            if block < self.block {
                break;
            }
        }
        Ok(skip)
    }

    async fn next(&mut self, sources: &ChunkListDesc, buf: Option<&mut [u8]>) -> BuckyResult<usize> {
        if let Some(cur) = &mut self.cur {
            let cur_remain = cur.range.end - cur.range.start;
            let (read, cur) = if cur_remain > self.block {
                if buf.is_some() {
                    buf.unwrap().copy_from_slice(&cur.cache(&self.reader, sources).await?[cur.range.start..cur.range.start + self.block]);
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
                        dst_buf[offset..offset + cur_remain].copy_from_slice(&cur.cache(&self.reader, sources).await?[cur.range.start..cur.range.start + cur_remain]);
                    }
                    offset += cur_remain;
                    cur.range.start += cur_remain;
                    if cur.range.start >= cur.range.end {
                        cur.index += 1;
                        if cur.index == self.stub.indices.end {
                            break None;
                        } else {
                            let cur_id = &sources.chunks()[cur.index as usize];
                            if cur.index == (self.stub.indices.end - 1) {
                                cur.range = if let Some(range) = &self.stub.last_range {
                                    range.start as usize..range.end as usize
                                } else {
                                    0..cur_id.len() as usize
                                };
                            } else {
                                let cur_id = &sources.chunks()[cur.index as usize];
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


    fn merge_inner(
        sources: &Vec<ChunkId>, 
        max_chunk_len: u64
    ) -> Vec<DsgChunkMergeStub> {
        #[derive(Clone)]
        struct CurIter {
            index: usize, 
            offset: u64, 
        }

        impl CurIter {
            fn new() -> Self {
                Self {
                    index: 0, 
                    offset: 0
                }
            }

            fn remain(&self, sources: &Vec<ChunkId>) -> u64 {
                sources[self.index as usize].len() as u64 - self.offset
            }

            fn offset_step(&mut self, sources: &Vec<ChunkId>, offset: u64) -> Option<DsgChunkMergeStub> {
                if self.index >= sources.len() {
                    None
                } else {
                    let remain = self.remain(sources);
                    if remain > offset {
                        let range = self.offset..self.offset + offset;
                        self.offset += offset;
                        Some(DsgChunkMergeStub::first(offset, self.index, Some(range)))
                    } else {
                        let stub = if self.offset == 0 {
                            DsgChunkMergeStub::first(remain, self.index, None)
                        } else {
                            DsgChunkMergeStub::first(remain, self.index, Some(self.offset..sources[self.index].len() as u64))
                        };
                        self.index += 1;
                        self.offset = 0;
                        Some(stub)
                    }
                }
            }

            fn offset(&mut self, sources: &Vec<ChunkId>, offset: u64) -> Option<DsgChunkMergeStub> {
                let mut stubs = vec![];
                let mut offset = offset;
                loop {
                    if let Some(stub) = self.offset_step(sources, offset) {
                        offset -= stub.len;
                        stubs.push(stub);
                    } else {
                        break;
                    }
                }
                if stubs.len() == 0 {
                    None
                } else if stubs.len() == 1 {
                    Some(stubs[0].clone())
                } else {
                    let mut stub = stubs[0].clone();
                    stub.len = stubs.iter().fold(0, |len, stub| len + stub.len);
                        
                    let last = stubs[stubs.len() - 1].clone();
                    stub.indices.end = last.indices.end;
                    stub.last_range = last.last_range.clone();
                    Some(stub)
                }
            }
        }

        let mut functions = vec![];
        let mut cur_iter = CurIter::new();
        let mut offset = 0;
        loop {
            if let Some(mut stub) = cur_iter.offset(sources, max_chunk_len) {
                stub.offset = offset;
                offset += stub.len;
                functions.push(stub);
            } else {
                break;
            }
        } 

        functions
    }

    pub fn merge(
        stack: &SharedCyfsStack, 
        sources: &Vec<ChunkId>, 
        split: u32
    ) -> DsgDataSourceStubObject {
        let max_chunk_len = Self::max_chunk_len(stack);
        NamedObjectBuilder::new(
            DsgDataSourceStubDesc {
                functions: Self::merge_inner(sources, max_chunk_len).into_iter()
                    .map(|chunks| DsgChunkFunctionMerge { key: None,  chunks, split }).collect()
            }, 
            DsgDataSourceStubBody {})
        .no_create_time().build()
    }

    fn header_len(stack: &SharedCyfsStack) -> usize {
        let version_len = 0u8.raw_measure(&None).unwrap();
        let key_len = AesKey::default().raw_measure(&None).unwrap();
        let encrypt_len = stack.local_device().desc().public_key().key_size();
        let header_len = version_len + encrypt_len;
        if header_len % key_len == 0 {
            header_len 
        } else {
            key_len * (header_len / key_len + 1)
        }
    }

    fn max_chunk_len(stack: &SharedCyfsStack) -> u64 {
        u32::max_value() as u64 - Self::header_len(stack) as u64
    }

    pub fn merge_with_key(
        stack: &SharedCyfsStack, 
        sources: &Vec<ChunkId>, 
        aes_key: AesKey, 
        split: u32
    ) -> DsgDataSourceStubObject {
        let max_chunk_len = Self::max_chunk_len(stack);
        NamedObjectBuilder::new(
            DsgDataSourceStubDesc {
                functions: Self::merge_inner(sources, max_chunk_len).into_iter()
                    .map(|chunks| DsgChunkFunctionMerge { key: Some(aes_key.clone()),  chunks, split }).collect()
            }, 
            DsgDataSourceStubBody {})
        .no_create_time().build()
    }

    pub async fn apply(
        &self, 
        stack: Arc<SharedCyfsStack>, 
        sources: ChunkListDesc, 
    ) -> BuckyResult<Vec<ChunkId>> {
        if self.is_unchanged() {
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
            let header_len = Self::header_len(stack.as_ref());

            for f in self.functions() {
                let mut buffer = vec![0u8; f.split as usize];
                
                if let Some(key) = &f.key {
                    let key_len = key.raw_measure(&None)?;

                    let mut reader = MergeReader::new(DsgStackChunkReader::new(stack.clone()).clone_as_reader(), &sources, f.chunks.clone(), key_len).await?;
                    
                    // first split with header
                    {   
                        let ptr = 0u8.raw_encode(buffer.as_mut_slice(), &None)?;
                   
                        let mut key_buf = vec![0u8; key_len];
                        let _ = key.raw_encode(key_buf.as_mut_slice(), &None)?;
                        let encrypt_len = stack.local_device().desc().public_key().encrypt(&key_buf.as_slice(), ptr)?;

                        let mut ptr = &mut buffer[header_len..]; 
                    
                        let mut total = header_len;
                        for _ in 0..((f.split as usize - header_len) / key_len) {
                            let read = reader.next(&sources, Some(ptr)).await?;
                            
                            total += read;
                            if read < key_len {
                                break;
                            }
                            ptr = &mut ptr[key_len..];
                        }
                        let enc_len = key.inplace_encrypt(&mut buffer[header_len..], total - header_len).unwrap();
                        let total = header_len + enc_len;
                        chunks.push(add_chunk(stack.as_ref(), &buffer[0..total]).await?);
                    }
                    
                  
                    loop {
                        let mut ptr = &mut buffer[..]; 
                        
                        let mut total = 0;
                        for _ in 0..(f.split as usize / key_len) {
                            let read = reader.next(&sources, Some(ptr)).await?;
                            total += read;
                            if read < key_len {
                                let enc_len = key.inplace_encrypt(&mut buffer[..], total).unwrap();
                                chunks.push(add_chunk(stack.as_ref(), &buffer[0..enc_len]).await?);
                                break;
                            }
                            ptr = &mut ptr[key_len..];
                        }
    
                        if total != 0 {
                            assert_eq!(total, f.split as usize);
                            let enc_len = key.inplace_encrypt(&mut buffer[..], total).unwrap();
                            assert_eq!(enc_len, total);
                            chunks.push(add_chunk(stack.as_ref(), &buffer[..]).await?);
                        } else {
                            break;
                        }
                    }
                } else {
                    let mut reader = MergeReader::new(DsgStackChunkReader::new(stack.clone()).clone_as_reader(), &sources, f.chunks.clone(), f.split as usize).await?;
                    loop {
                        let total = reader.next(&sources, Some(&mut buffer[header_len..])).await?;
                        if total != 0 {
                            chunks.push(add_chunk(stack.as_ref(), &buffer[header_len..(header_len + total)]).await?);
                        } else {
                            break;
                        }
                    }
                }
            }

            Ok(chunks)
        }
    }

    pub async fn restore(
        &self, 
        stack: &SharedCyfsStack, 
        backup: &Vec<ChunkId>
    ) -> BuckyResult<Vec<ChunkId>> {
        if self.is_unchanged() {
            Ok(backup.clone())
        } else {
            unimplemented!()
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
            let sample_offset = merged.offset_of(sample.chunk_index as usize).unwrap() + sample.offset_in_chunk;
            let func_index = (sample_offset / u32::max_value() as u64) as usize;
            let func = self.functions().get(func_index)
                .ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "no function"))?;
            let func_offset = func_index as u64 * u32::max_value() as u64;
            let mut offset_in_func = sample_offset - func_offset;
            let header_len = Self::header_len(stack) as u64;
            if offset_in_func < header_len {
                offset_in_func = header_len;
            }
            let offset_in_func = offset_in_func - header_len;
            let key_len = AesKey::default().raw_measure(&None).unwrap();
            let mut reader = MergeReader::new(reader.clone_as_reader(), &sources, func.chunks.clone(), key_len).await?;
            let mut result = vec![0u8; sample.sample_len as usize];
            let mut result_offset = 0; 
            if offset_in_func != reader.skip(&sources, (offset_in_func / key_len as u64) as usize).await? {

            } 
            let mut buffer = vec![0u8; key_len];
          
            let mut remain = sample.sample_len as usize;
            let _ = loop {
                let read = reader.next(&sources, Some(buffer.as_mut_slice())).await?;
                if let Some(key) = &func.key {
                    let _ = key.inplace_encrypt(&mut buffer[..], read).unwrap();
                }
                if read == 0 {
                    //error
                    break Err(BuckyError::new(BuckyErrorCode::OutOfLimit, "")); 
                }
                if read >= remain {
                    result[result_offset..result_offset + remain].copy_from_slice(&buffer[..remain]);
                    break Ok(());
                } else {
                    result[result_offset..result_offset + read].copy_from_slice(&buffer[..read]);
                    remain -= read;
                    result_offset += read;
                }
            }?;
            
            Ok(Box::new(Cursor::new(result)))
        }
    }
}
