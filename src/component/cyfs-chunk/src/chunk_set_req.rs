use cyfs_base::*;

// #[derive(RawEncode, RawDecode)]
pub struct ChunkSetReq {
    source_device_id: DeviceId,
    chunk_id: ChunkId,
    data: SizedOwnedData<SizeU32>,
    // pub sign: Signature,
}

impl ChunkSetReq {
    pub fn source_device_id(&self) -> &DeviceId {
        &self.source_device_id
    }

    pub fn chunk_id(&self) -> &ChunkId {
        &self.chunk_id
    }

    pub fn data(&self) -> &[u8] {
        &self.data.as_slice()
    }

    pub fn sign(
        _source_signer: &PrivateKey,
        source_device_id: &DeviceId,
        chunk_id: &ChunkId,
        data: Vec<u8>,
    ) -> BuckyResult<ChunkSetReq> {
        // TODO
        Ok(Self {
            source_device_id: source_device_id.clone(),
            chunk_id: chunk_id.clone(),
            data: SizedOwnedData::<SizeU32>::from(data),
        })
    }

    pub fn verify(&self, _source_public_key: &PublicKey) -> bool {
        // TODO
        true
    }
}

impl RawEncode for ChunkSetReq {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let size = 0
            + self.source_device_id.raw_measure(purpose)?
            + self.chunk_id.raw_measure(purpose)?
            + self.data.raw_measure(purpose)?;

        Ok(size)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let size = self.raw_measure(purpose).unwrap();
        if buf.len() < size {
            let msg = format!("not enough buffer for encode ChunkSetSeq, except={}, got={}", size, buf.len());
            log::error!("{}", msg);

            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                msg,
            ));
        }

        let buf = self.source_device_id.raw_encode(buf, purpose)?;
        let buf = self.chunk_id.raw_encode(buf, purpose)?;
        let buf = self.data.raw_encode(buf, purpose)?;
        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for ChunkSetReq {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (source_device_id, buf) = DeviceId::raw_decode(buf)?;
        let (chunk_id, buf) = ChunkId::raw_decode(buf)?;
        let (data, buf) = SizedOwnedData::raw_decode(buf)?;

        Ok((
            Self {
                source_device_id,
                chunk_id,
                data,
            },
            buf,
        ))
    }
}
