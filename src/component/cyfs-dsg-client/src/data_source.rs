use std::{
    sync::Arc,
};
use std::io::SeekFrom;
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
use cyfs_chunk_lib::{Chunk, MemChunk};
use cyfs_lib::*;
use crate::{
    obj_id,
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

#[derive(Clone)]
pub struct DsgDataSourceStubDesc {
    pub key: Option<AesKey>,
    pub chunks: Vec<ChunkId>,
    pub split: u32
}
//
// impl TryFrom<&DsgDataSourceStubDesc> for protos::DataSourceStubDesc {
//     type Error = BuckyError;
//
//     fn try_from(rust: &DsgDataSourceStubDesc) -> BuckyResult<Self> {
//         let mut proto = protos::DataSourceStubDesc::new();
//         proto.set_functions(ProtobufCodecHelper::encode_nested_list(&rust.functions)?);
//         Ok(proto)
//     }
// }
//
// impl TryFrom<protos::DataSourceStubDesc> for DsgDataSourceStubDesc {
//     type Error = BuckyError;
//
//     fn try_from(mut proto: protos::DataSourceStubDesc) -> BuckyResult<Self> {
//         Ok(Self {
//             functions: ProtobufCodecHelper::decode_nested_list(proto.take_functions())?
//         })
//     }
// }
//
// impl_default_protobuf_raw_codec!(DsgDataSourceStubDesc, protos::DataSourceStubDesc);

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

#[derive(Clone)]
enum MergeWriterState {
    Header,
    Chunks
}


struct MergeWriter {
    state: MergeWriterState,
    source: ChunksReader,
    header: EncryptHeader,
    aes_key: AesKey,
}

#[derive(RawDecode, RawEncode)]
struct EncryptHeader {
    flag: u8,
    encrypted_key: Vec<u8>,
    chunk_size_list: Vec<u32>,
}

impl EncryptHeader {
    fn len(&self) -> BuckyResult<usize> {
        let len = self.raw_measure(&None)?;
        let enc_block = ChunkAesCodec::enc_block_len();
        if len % enc_block == 0 {
            Ok(len)
        } else {
            Ok((len / enc_block + 1) * enc_block)
        }
    }

    fn encode<'a>(&self, buf: &'a mut [u8]) -> BuckyResult<&'a mut [u8]> {
        let mut len = self.raw_measure(&None)?;
        let buf = self.raw_encode(buf, &None)?;
        let enc_block = ChunkAesCodec::enc_block_len();
        if len % enc_block != 0 {
            let padding_len = enc_block - len % enc_block;
            let padding = vec![0u8; padding_len];
            buf[len..len + padding_len].copy_from_slice(padding.as_slice());
            len += padding_len;
        }
        Ok(&mut buf[len..])
    }
}

impl MergeWriter {
    fn new(header: EncryptHeader, source: ChunksReader, aes_key: AesKey) -> Self {
        Self {
            state: MergeWriterState::Header,
            source,
            header,
            aes_key
        }
    }

    #[async_recursion]
    async fn next_block_inner(&mut self, block: &mut [u8]) -> BuckyResult<usize> {
        let enc_block = ChunkAesCodec::enc_block_len();
        match self.state.clone() {
            MergeWriterState::Header => unreachable!(),
            MergeWriterState::Chunks => {
                let mut total = 0;
                let mut ptr = &mut block[..];
                loop {
                    let read = self.source.read_async(ptr).await?;
                    total += read;
                    if read == 0 || ptr[read..].len() == 0 {
                        break;
                    }
                    ptr = &mut ptr[read..];
                }

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

                let enc_total = ChunkAesCodec::enc_block(&self.aes_key, block, total)?;
                assert_eq!(enc_total, total);

                Ok(total)
            }
        }
    }

    async fn next_block(&mut self, block: &mut [u8]) -> BuckyResult<usize> {
        match self.state.clone() {
            MergeWriterState::Header => {
                let buf = self.header.encode(block)?;

                self.state = MergeWriterState::Chunks;

                self.next_block_inner(buf).await
            },
            MergeWriterState::Chunks => {
                self.next_block_inner(block).await
            }
        }
    }
}

pub struct ChunksReader {
    stack: Arc<SharedCyfsStack>,
    chunks: Vec<(usize, ChunkId)>,
    len: usize,
    pos: u64,
    cache: Option<(ChunkId, Box<dyn Chunk>)>
}

impl ChunksReader {
    pub fn new(stack: Arc<SharedCyfsStack>, chunks: &Vec<ChunkId>) -> Self {
        let mut pos = 0;
        let mut list = Vec::new();
        for chunk_id in chunks.iter() {
            list.push((pos, chunk_id.clone()));
            pos += chunk_id.len();
        }
        Self {
            stack,
            chunks: list,
            len: pos,
            pos: 0,
            cache: None
        }
    }

    fn get_chunk_id_by_pos(&self, pos: u64) -> BuckyResult<(u64, ChunkId)> {
        let mut cur_pos = 0;
        for (_, item) in self.chunks.iter() {
            if cur_pos <= pos && cur_pos + item.len() as u64 > pos {
                return Ok((cur_pos, item.clone()));
            } else {
                cur_pos += item.len() as u64;
            }
        }
        Err(BuckyError::new(BuckyErrorCode::NotFound, "can't find chunkid"))
    }

    async fn get_chunk(&self, chunk_id: &ChunkId) -> BuckyResult<Box<dyn Chunk>> {
        let mut resp = self.stack.ndn_service().get_data(NDNGetDataOutputRequest {
            common: NDNOutputRequestCommon {
                req_path: None,
                dec_id: None,
                level: NDNAPILevel::NDC,
                target: None,
                referer_object: vec![],
                flags: 0
            },
            object_id: chunk_id.object_id(),
            range: None,
            inner_path: None
        }).await?;

        let mut chunk_data = vec![];
        let _ = resp.data.read_to_end(&mut chunk_data).await.map_err(|e| {
            let msg = format!("get chunk err {}", e);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        Ok(Box::new(MemChunk::from(chunk_data)))
    }

    async fn get_chunk_by_pos(&mut self, pos: u64) -> BuckyResult<(u64, ChunkId, Box<dyn Chunk>)> {
        let (chunk_pos, chunk_id) = self.get_chunk_id_by_pos(pos)?;
        if self.cache.is_some() {
            let cache = self.cache.take().unwrap();
            if &cache.0 == &chunk_id {
                return Ok((chunk_pos, chunk_id, cache.1))
            }
        }
        let chunk = self.get_chunk(&chunk_id).await?;
        Ok((chunk_pos, chunk_id.clone(), chunk))
    }

    pub async fn read_async(&mut self, buf: &mut [u8]) -> BuckyResult<usize> {
        let mut tmp_buf = buf;
        let mut read_len = 0;
        if self.pos >= self.len as u64 {
            return Ok(0);
        }
        loop {
            let (chunk_pos, chunk_id, mut chunk) = self.get_chunk_by_pos(self.pos).await.map_err(|e| {
                let msg = format!("get_chunk_by_pos {} failed.err {}", self.pos, e);
                println!("{}", msg.as_str());
                log::error!("{}", msg.as_str());
                std::io::Error::new(std::io::ErrorKind::Other, msg)
            })?;

            let chunk_offset = self.pos - chunk_pos;
            chunk.seek(SeekFrom::Start(chunk_offset)).await?;
            let read_size = chunk.read(tmp_buf).await?;
            tmp_buf = &mut tmp_buf[read_size..];
            self.pos += read_size as u64;
            read_len += read_size;
            self.cache = Some((chunk_id, chunk));

            if tmp_buf.len() == 0 || self.pos >= self.len as u64 {
                break;
            }
        }

        Ok(read_len)
    }

    pub async fn seek_async(&mut self, pos: SeekFrom) -> BuckyResult<u64> {
        let this = self;
        match pos {
            SeekFrom::Start(pos) => {
                this.pos = pos;
                Ok(pos)
            },
            SeekFrom::End(pos) => {
                if this.len as i64 + pos < 0 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, format!("seek failed")));
                }
                this.pos = (this.len as i64 + pos) as u64;
                Ok(this.pos as u64)
            },
            SeekFrom::Current(pos) => {
                if this.pos as i64 + pos < 0 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, format!("seek failed")));
                }
                this.pos = (this.pos as i64 + pos) as u64;
                Ok(this.pos)
            }
        }
    }
}

pub struct ChunkAesCodec;

impl ChunkAesCodec {
    pub fn enc_block_len() -> usize {
        <Aes256 as BlockCipher>::BlockSize::to_usize()
    }


    fn enc_block(key: &AesKey, buffer: &mut [u8], len: usize) -> BuckyResult<usize> {
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

    fn dec_block(key: &AesKey, buffer: &mut [u8], len: usize) -> BuckyResult<usize> {
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

    fn create_header(stack: &SharedCyfsStack, aes_key: &AesKey, chunks: &Vec<ChunkId>) -> BuckyResult<EncryptHeader> {
        let chunk_size_list = chunks.iter().map(|i| i.len() as u32).collect();

        let key_len = aes_key.raw_measure(&None)?;
        let mut key_buf = vec![0u8; key_len];
        let _ = aes_key.raw_encode(key_buf.as_mut_slice(), &None)?;
        let mut encrypt_buf = [0u8; 4096];
        let encrypt_len = stack.local_device().desc().public_key().encrypt(&key_buf.as_slice(), encrypt_buf.as_mut_slice())?;
        let encrypted_key = encrypt_buf[0..encrypt_len].to_vec();

        let header = EncryptHeader {
            flag: 0,
            encrypted_key,
            chunk_size_list
        };
        Ok(header)
    }

    async fn add_chunk(stack: &SharedCyfsStack, buffer: &[u8]) -> BuckyResult<ChunkId> {
        let mut hasher = Sha256::new();
        hasher.input(buffer);
        let chunk_id = ChunkId::new(&hasher.result().into(), buffer.len() as u32);
        let _ = stack.ndn_service().put_data(NDNPutDataOutputRequest::new(NDNAPILevel::NDC, chunk_id.object_id(), buffer.len() as u64, Box::new(Cursor::new(Vec::from(buffer))))).await?;
        Ok(chunk_id)
    }

    pub async fn encode(
        stack: Arc<SharedCyfsStack>,
        sources: Vec<ChunkId>,
        aes_key: AesKey,
        split: u32
    ) -> BuckyResult<Vec<ChunkId>> {
        let mut chunks = vec![];

        let mut buffer = vec![0u8; split as usize];

        let header = Self::create_header(&stack, &aes_key, &sources)?;
        let reader = ChunksReader::new(stack.clone(), &sources);
        let mut writer = MergeWriter::new(header, reader, aes_key);

        loop {
            let written = writer.next_block(&mut buffer[..]).await?;
            if written == 0 {
                break;
            }
            chunks.push(Self::add_chunk(stack.as_ref(), &buffer[..written]).await?);
        }


        Ok(chunks)
    }

    pub async fn decode(
        stack: Arc<SharedCyfsStack>,
        merged: Vec<ChunkId>,
    ) -> BuckyResult<Vec<ChunkId>> {
        let split = merged[0].len();
        let mut reader = ChunksReader::new(stack.clone(), &merged);
        let mut chunks = vec![];

        let mut buffer = vec![0u8; split as usize];
        let read_size = reader.read_async(&mut buffer).await?;
        let header = EncryptHeader::clone_from_slice(buffer.as_slice())?;
        let header_len = header.len()?;

        let aes_key = AesKey::random();

        let mut ptr: &mut [u8] = &mut buffer[header_len..read_size];
        let mut data_len = read_size - header_len;
        Self::dec_block(&aes_key, ptr, data_len)?;
        for chunk_size in header.chunk_size_list.iter() {
            if (*chunk_size as usize) <= data_len {
                let chunk_id = Self::add_chunk(&stack, &ptr[..*chunk_size as usize]).await?;
                chunks.push(chunk_id);
                data_len = data_len - *chunk_size as usize;
                ptr = &mut ptr[*chunk_size as usize..];
            } else {
                let mut chunk = vec![0u8; *chunk_size as usize];
                chunk[..data_len].copy_from_slice(ptr);
                let mut chunk_ptr = &mut chunk[data_len..];
                while chunk_ptr.len() > 0 {
                    ptr = &mut buffer[..];
                    let read_size = reader.read_async(ptr).await?;
                    if read_size == 0 {
                        return Err(BuckyError::new(BuckyErrorCode::Failed, "encrypt data err"));
                    }
                    Self::dec_block(&aes_key, &mut ptr, read_size)?;
                    if read_size >= chunk_ptr.len() {
                        let need_len = chunk_ptr.len();
                        chunk_ptr.copy_from_slice(&ptr[..need_len]);
                        data_len = read_size - need_len;
                        ptr = &mut ptr[need_len..];
                        chunk_ptr = &mut chunk_ptr[need_len..];
                    } else {
                        chunk_ptr[..read_size].copy_from_slice(&ptr[..read_size]);
                        chunk_ptr = &mut chunk_ptr[read_size..];
                    }
                }

                let chunk_id = Self::add_chunk(&stack, chunk.as_slice()).await?;
                chunks.push(chunk_id);
            }
        }

        Ok(chunks)
    }
}
