use super::common::*;

pub struct AckAckTunnel {
    seq: TempSeq,
}

impl Package for AckAckTunnel {
    fn version(&self) -> u8 {
        0
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::AckAckTunnel
    }
}

impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for AckAckTunnel {
    fn raw_measure_with_context(
        &self,
        _merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<usize, BuckyError> {
        unimplemented!()
    }

    fn raw_encode_with_context<'a>(
        &self,
        enc_buf: &'a mut [u8],
        merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Encode::<Self, Context>::new(enc_buf, merge_context)?;
        let _buf = context.check_encode(buf, "seq", &self.seq, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context> for AckAckTunnel {
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (seq, buf) = context.check_decode(buf, "seq", flags.next())?;
        Ok((
            Self {
                seq,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_ack_ack_tunnel() {
    use crate::interface::udp;

    let src = AckAckTunnel {
        seq: TempSeq::default(),
    };

    let mut buf = [0u8; udp::MTU];
    let remain = src
        .raw_encode_with_context(&mut buf, &mut merge_context::OtherEncode::default(), &None)
        .unwrap();
    let remain = remain.len();

    let dec = &buf[..buf.len() - remain];
    let (cmd, dec) = u8::raw_decode(dec)
        .map(|(code, dec)| (PackageCmdCode::try_from(code).unwrap(), dec))
        .unwrap();
    assert_eq!(cmd, PackageCmdCode::AckAckTunnel);
    let (dst, _) =
        AckAckTunnel::raw_decode_with_context(&dec, &mut merge_context::OtherDecode::default())
            .unwrap();

    assert_eq!(dst.seq, src.seq);
}

pub struct PingTunnel {
    pub package_id: u32,
    pub send_time: Timestamp,
    pub recv_data: u64,
}

impl Package for PingTunnel {
    fn version(&self) -> u8 {
        0
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::PingTunnel
    }
}

impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for PingTunnel {
    fn raw_measure_with_context(
        &self,
        _merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<usize, BuckyError> {
        unimplemented!()
    }

    fn raw_encode_with_context<'a>(
        &self,
        enc_buf: &'a mut [u8],
        merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Encode::<Self, Context>::new(enc_buf, merge_context)?;
        let buf = context.encode(buf, &self.package_id, flags.next())?;
        let buf = context.encode(buf, &self.send_time, flags.next())?;
        let _buf = context.encode(buf, &self.recv_data, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context> for PingTunnel {
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (package_id, buf) = context.decode(buf, "PingTunnel.package_id", flags.next())?;
        let (send_time, buf) = context.decode(buf, "PingTunnel.send_time", flags.next())?;
        let (recv_data, buf) = context.decode(buf, "PingTunnel.recv_data", flags.next())?;

        Ok((
            Self {
                package_id,
                send_time,
                recv_data,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_ping_tunnel() {
    use crate::interface::udp;

    let src = PingTunnel {
        package_id: rand::random::<u32>(),
        send_time: bucky_time_now(),
        recv_data: rand::random::<u64>(),
    };

    let mut buf = [0u8; udp::MTU];
    let remain = src
        .raw_encode_with_context(&mut buf, &mut merge_context::OtherEncode::default(), &None)
        .unwrap();
    let remain = remain.len();

    let dec = &buf[..buf.len() - remain];
    let (cmd, dec) = u8::raw_decode(dec)
        .map(|(code, dec)| (PackageCmdCode::try_from(code).unwrap(), dec))
        .unwrap();
    assert_eq!(cmd, PackageCmdCode::PingTunnel);
    let (dst, _) =
        PingTunnel::raw_decode_with_context(&dec, &mut merge_context::OtherDecode::default())
            .unwrap();

    assert_eq!(dst.package_id, src.package_id);
    assert_eq!(dst.send_time, src.send_time);
    assert_eq!(dst.recv_data, src.recv_data);
}

pub struct PingTunnelResp {
    pub ack_package_id: u32,
    pub send_time: Timestamp,
    pub recv_data: u64,
}

impl Package for PingTunnelResp {
    fn version(&self) -> u8 {
        0
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::PingTunnelResp
    }
}

impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for PingTunnelResp {
    fn raw_measure_with_context(
        &self,
        _merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<usize, BuckyError> {
        unimplemented!()
    }

    fn raw_encode_with_context<'a>(
        &self,
        enc_buf: &'a mut [u8],
        merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Encode::<Self, Context>::new(enc_buf, merge_context)?;
        let buf = context.encode(buf, &self.ack_package_id, flags.next())?;
        let buf = context.encode(buf, &self.send_time, flags.next())?;
        let _buf = context.encode(buf, &self.recv_data, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context>
    for PingTunnelResp
{
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (ack_package_id, buf) = context.decode(buf, "PingTunnelResp.ack_package_id", flags.next())?;
        let (send_time, buf) = context.decode(buf, "PingTunnelResp.send_time", flags.next())?;
        let (recv_data, buf) = context.decode(buf, "PingTunnelResp.recv_data", flags.next())?;

        Ok((
            Self {
                ack_package_id,
                send_time,
                recv_data,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_ping_tunnel_resp() {
    use crate::interface::udp;

    let src = PingTunnelResp {
        ack_package_id: rand::random::<u32>(),
        send_time: bucky_time_now(),
        recv_data: rand::random::<u64>(),
    };

    let mut buf = [0u8; udp::MTU];
    let remain = src
        .raw_encode_with_context(&mut buf, &mut merge_context::OtherEncode::default(), &None)
        .unwrap();
    let remain = remain.len();

    let dec = &buf[..buf.len() - remain];
    let (cmd, dec) = u8::raw_decode(dec)
        .map(|(code, dec)| (PackageCmdCode::try_from(code).unwrap(), dec))
        .unwrap();
    assert_eq!(cmd, PackageCmdCode::PingTunnelResp);
    let (dst, _) =
        PingTunnelResp::raw_decode_with_context(&dec, &mut merge_context::OtherDecode::default())
            .unwrap();

    assert_eq!(dst.ack_package_id, src.ack_package_id);
    assert_eq!(dst.send_time, src.send_time);
    assert_eq!(dst.recv_data, src.recv_data);
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum DatagramType {
    Data = 1,
}

impl RawEncode for DatagramType {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(1)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        (*self as u8).raw_encode(buf, purpose)
    }
}

impl<'de> RawDecode<'de> for DatagramType {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (code, buf) = u8::raw_decode(buf)?;
        if code == 1 {
            Ok((DatagramType::Data, buf))
        } else {
            Err(BuckyError::new(
                BuckyErrorCode::InvalidData,
                "unknown datagram-type",
            ))
        }
    }
}

#[derive(Clone)]
pub struct Datagram {
    pub to_vport: u16,
    pub from_vport: u16,
    pub dest_zone: Option<u32>,
    pub hop_limit: Option<u8>,
    pub sequence: Option<TempSeq>,
    pub piece: Option<(u8, u8)>, // index/count
    pub send_time: Option<Timestamp>,
    pub create_time: Option<Timestamp>, // <TODO>这个是啥？协议文档上没有
    pub author_id: Option<DeviceId>,
    pub author: Option<Device>,
    // pub data_sign: Option<Signature>, // <TODO>暂时不清楚怎么签名，先把C版本搬过来
    pub inner_type: DatagramType,
    pub data: TailedOwnedData, // TailedSharedData<'a>,
}

impl Datagram {
    pub fn fragment_len(&self, mtu: usize, plaintext: bool) -> usize {
        let box_header_len = 8; //mixhash
        let dynamic_header_len = 3;
        let mut datagram_header_len = 0;
        let mut dynamic_package_len;
        let aes_width = 16;
        let piece_field_len = 2;

        datagram_header_len += 2;
        datagram_header_len += 2;
        if self.dest_zone.is_some() {
            datagram_header_len += 4;
        }
        if self.hop_limit.is_some() {
            datagram_header_len += 1;
        }
        if self.sequence.is_some() {
            datagram_header_len += 4;
        }
        if self.piece.is_some() {//must be none
            datagram_header_len += 2;
        }
        if self.send_time.is_some() {
            datagram_header_len += 8;
        }
        if self.create_time.is_some() {
            datagram_header_len += 8;
        }
        if self.author_id.is_some() {
            datagram_header_len += std::mem::size_of::<DeviceId>();
        }
        if self.author.is_some() {
            datagram_header_len += std::mem::size_of::<Device>();
        }
        datagram_header_len += 1;

        dynamic_package_len = dynamic_header_len + datagram_header_len + self.data.as_ref().len();
        if !plaintext {
            dynamic_package_len = dynamic_package_len/aes_width*aes_width+aes_width;
        }

        if box_header_len + dynamic_package_len <= mtu {
            0
        } else {
            dynamic_package_len = mtu-box_header_len;
            if !plaintext {
                dynamic_package_len = dynamic_package_len/aes_width*aes_width-aes_width;
            }
            let data_len = dynamic_package_len-dynamic_header_len-datagram_header_len-piece_field_len;
            data_len
        }
    }
}

impl Package for Datagram {
    fn version(&self) -> u8 {
        0
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::Datagram
    }
}

impl std::fmt::Display for Datagram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Datagram:{{sequence:{:?}, create_time:{:?}, data:{}}}",
            self.sequence,
            self.create_time,
            self.data.as_ref().len()
        )
    }
}

impl RawEncode for Datagram {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let len = u8::raw_bytes().unwrap()
            + u16::raw_bytes().unwrap()
            + self.to_vport.raw_measure(purpose)?
            + self.from_vport.raw_measure(purpose)?
            + if self.dest_zone.is_none() {
                0
            } else {
                self.dest_zone.as_ref().unwrap().raw_measure(purpose)?
            }
            + if self.hop_limit.is_none() {
                0
            } else {
                self.hop_limit.as_ref().unwrap().raw_measure(purpose)?
            }
            + if self.piece.is_none() {
                0
            } else {
                self.piece.as_ref().unwrap().raw_measure(purpose)?
            }
            + if self.sequence.is_none() {
                0
            } else {
                self.sequence.as_ref().unwrap().raw_measure(purpose)?
            }
            + if self.send_time.is_none() {
                0
            } else {
                self.send_time.as_ref().unwrap().raw_measure(purpose)?
            }
            + if self.create_time.is_none() {
                0
            } else {
                self.create_time.as_ref().unwrap().raw_measure(purpose)?
            }
            + if self.author_id.is_none() {
                0
            } else {
                self.author_id.as_ref().unwrap().raw_measure(purpose)?
            }
            + if self.author.is_none() {
                0
            } else {
                self.author.as_ref().unwrap().raw_measure(purpose)?
            }
            + self.inner_type.raw_measure(purpose)?
            + self.data.raw_measure(purpose)?;
        Ok(len)
    }

    fn raw_encode<'a>(
        &self,
        _buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        unimplemented!()
    }
}

impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for Datagram {
    fn raw_measure_with_context(
        &self,
        _merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<usize, BuckyError> {
        unimplemented!()
    }

    fn raw_encode_with_context<'a>(
        &self,
        enc_buf: &'a mut [u8],
        merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Encode::<Self, Context>::new(enc_buf, merge_context)?;
        let buf = context.encode(buf, &self.to_vport, flags.next())?;
        let buf = context.encode(buf, &self.from_vport, flags.next())?;
        let buf = context.option_encode(buf, &self.dest_zone, flags.next())?;
        let buf = context.option_encode(buf, &self.hop_limit, flags.next())?;
        let buf = context.option_encode(buf, &self.sequence, flags.next())?;
        let buf = context.option_encode(buf, &self.piece, flags.next())?;
        let buf = context.option_encode(buf, &self.send_time, flags.next())?;
        let buf = context.option_encode(buf, &self.create_time, flags.next())?;
        let buf = context.option_encode(buf, &self.author_id, flags.next())?;
        let buf = context.option_encode(buf, &self.author, flags.next())?;
        let buf = context.encode(buf, &self.inner_type, flags.next())?;
        let _buf = context.encode(buf, &self.data, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context> for Datagram {
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (to_vport, buf) = context.decode(buf, "Datagram.to_vport", flags.next())?;
        let (from_vport, buf) = context.decode(buf, "Datagram.from_vport", flags.next())?;
        let (dest_zone, buf) = context.option_decode(buf, flags.next())?;
        let (hop_limit, buf) = context.option_decode(buf, flags.next())?;
        let (sequence, buf) = context.option_decode(buf, flags.next())?;
        let (piece, buf) = context.option_decode(buf, flags.next())?;
        let (send_time, buf) = context.option_decode(buf, flags.next())?;
        let (create_time, buf) = context.option_decode(buf, flags.next())?;
        let (author_id, buf) = context.option_decode(buf, flags.next())?;
        let (author, buf) = context.option_decode(buf, flags.next())?;
        let (inner_type, buf) = context.decode(buf, "Datagram.inner_type", flags.next())?;
        let (data, buf) = context.decode(buf, "Datagram.data", flags.next())?;

        Ok((
            Self {
                to_vport,
                from_vport,
                sequence,
                dest_zone,
                hop_limit,
                piece,
                send_time,
                create_time,
                author_id,
                author,
                inner_type,
                data,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_datagram() {
    use crate::interface::udp;

    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let from_device = Device::new(
        None,
        UniqueId::default(),
        vec![],
        vec![],
        vec![],
        private_key.public(),
        Area::default(),
        DeviceCategory::PC,
    )
    .build();

    let data = "hello".as_bytes().to_vec();
    let src = Datagram {
        to_vport: rand::random::<u16>(),
        from_vport: rand::random::<u16>(),
        dest_zone: Some(rand::random::<u32>()),
        hop_limit: Some(rand::random::<u8>()),
        sequence: Some(TempSeq::from(rand::random::<u32>())),
        piece: Some((rand::random::<u8>(), rand::random::<u8>())),
        send_time: Some(bucky_time_now()),
        create_time: Some(bucky_time_now()),
        author_id: Some(from_device.desc().device_id()),
        author: Some(from_device),
        inner_type: DatagramType::Data,
        data: TailedOwnedData::from(data),
    };

    let mut buf = [0u8; udp::MTU];
    let remain = src
        .raw_encode_with_context(&mut buf, &mut merge_context::OtherEncode::default(), &None)
        .unwrap();
    let remain = remain.len();

    let dec = &buf[..buf.len() - remain];
    let (cmd, dec) = u8::raw_decode(dec)
        .map(|(code, dec)| (PackageCmdCode::try_from(code).unwrap(), dec))
        .unwrap();
    assert_eq!(cmd, PackageCmdCode::Datagram);
    let (dst, _) =
        Datagram::raw_decode_with_context(&dec, &mut merge_context::OtherDecode::default())
            .unwrap();

    assert_eq!(dst.to_vport, src.to_vport);
    assert_eq!(dst.from_vport, src.from_vport);
    assert_eq!(dst.dest_zone, src.dest_zone);
    assert_eq!(dst.hop_limit, src.hop_limit);
    assert_eq!(dst.sequence, src.sequence);
    assert_eq!(dst.piece, src.piece);
    assert_eq!(dst.send_time, src.send_time);
    assert_eq!(dst.create_time, src.create_time);
    assert_eq!(dst.author_id, src.author_id);

    let dst_author = dst.author.unwrap().raw_hash_encode().unwrap();
    let src_author = src.author.unwrap().raw_hash_encode().unwrap();
    assert_eq!(dst_author, src_author);

    let dst_inner_type = dst.inner_type.to_hex().unwrap();
    let src_inner_type = src.inner_type.to_hex().unwrap();
    assert_eq!(dst_inner_type, src_inner_type);

    let dst_data = dst.data.to_string();
    let src_dsta = src.data.to_string();
    assert_eq!(dst_data, src_dsta);
}

#[derive(Clone)]
pub struct SessionSynInfo {
    pub sequence: TempSeq,
    pub from_session_id: IncreaseId,
    pub to_vport: u16,
}

impl RawEncode for SessionSynInfo {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        Ok(self.sequence.raw_measure(purpose)?
            + self.from_session_id.raw_measure(purpose)?
            + self.to_vport.raw_measure(purpose)?)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let buf = self.sequence.raw_encode(buf, purpose)?;
        let buf = self.from_session_id.raw_encode(buf, purpose)?;
        let buf = self.to_vport.raw_encode(buf, purpose)?;
        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for SessionSynInfo {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let (sequence, buf) = TempSeq::raw_decode(buf)?;
        let (from_session_id, buf) = IncreaseId::raw_decode(buf)?;
        let (to_vport, buf) = u16::raw_decode(buf)?;
        Ok((
            Self {
                sequence,
                from_session_id,
                to_vport,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_session_syn_info() {
    use crate::interface::udp;

    let src = SessionSynInfo {
        sequence: TempSeq::from(rand::random::<u32>()),
        from_session_id: IncreaseId::default(),
        to_vport: rand::random::<u16>(),
    };

    let mut buf = [0u8; udp::MTU];
    let remain = src.raw_encode(&mut buf, &None).unwrap();

    let remain = remain.len();
    let dec = &buf[..buf.len() - remain];
    let (dst, _) = SessionSynInfo::raw_decode(dec).unwrap();

    assert_eq!(dst.sequence, src.sequence);
    assert_eq!(dst.from_session_id, src.from_session_id);
    assert_eq!(dst.to_vport, src.to_vport)
}

#[derive(Clone)]
pub struct SessionDataPackageIdPart {
    pub package_id: IncreaseId,
    pub total_recv: u64,
}

impl RawEncode for SessionDataPackageIdPart {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        Ok(self.package_id.raw_measure(purpose)? + self.total_recv.raw_measure(purpose)?)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let buf = self.package_id.raw_encode(buf, purpose)?;
        let buf = self.total_recv.raw_encode(buf, purpose)?;
        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for SessionDataPackageIdPart {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let (package_id, buf) = IncreaseId::raw_decode(buf)?;
        let (total_recv, buf) = u64::raw_decode(buf)?;

        Ok((
            Self {
                package_id,
                total_recv,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_session_data_package_id_part() {
    use crate::interface::udp;

    let src = SessionDataPackageIdPart {
        package_id: IncreaseId::default(),
        total_recv: rand::random::<u64>(),
    };

    let mut buf = [0u8; udp::MTU];
    let remain = src.raw_encode(&mut buf, &None).unwrap();

    let remain = remain.len();
    let dec = &buf[..buf.len() - remain];
    let (dst, _) = SessionDataPackageIdPart::raw_decode(dec).unwrap();

    assert_eq!(dst.package_id, src.package_id);
    assert_eq!(dst.total_recv, src.total_recv);
}

#[derive(Clone)]
pub struct StreamRange {
    pub pos: u64,
    pub length: u32,
}

impl RawEncode for StreamRange {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        Ok(self.pos.raw_measure(purpose)? + self.length.raw_measure(purpose)?)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let buf = self.pos.raw_encode(buf, purpose)?;
        let buf = self.length.raw_encode(buf, purpose)?;
        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for StreamRange {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let (pos, buf) = u64::raw_decode(buf)?;
        let (length, buf) = u32::raw_decode(buf)?;

        Ok((Self { pos, length }, buf))
    }
}

#[test]
fn encode_protocol_stream_range() {
    use crate::interface::udp;

    let src = StreamRange {
        pos: rand::random::<u64>(),
        length: rand::random::<u32>(),
    };

    let mut buf = [0u8; udp::MTU];
    let remain = src.raw_encode(&mut buf, &None).unwrap();

    let remain = remain.len();
    let dec = &buf[..buf.len() - remain];
    let (dst, _) = StreamRange::raw_decode(dec).unwrap();

    assert_eq!(dst.pos, src.pos);
    assert_eq!(dst.length, src.length);
}

#[derive(Clone)]
pub struct StreamRanges(Vec<StreamRange>);

impl From<Vec<StreamRange>> for StreamRanges {
    fn from(v: Vec<StreamRange>) -> Self {
        Self(v)
    }
}

impl AsRef<Vec<StreamRange>> for StreamRanges {
    fn as_ref(&self) -> &Vec<StreamRange> {
        &self.0
    }
}

impl AsMut<Vec<StreamRange>> for StreamRanges {
    fn as_mut(&mut self) -> &mut Vec<StreamRange> {
        &mut self.0
    }
}

impl RawEncode for StreamRanges {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        let mut total = 0 as usize;
        total += 1; //count u8
        for s in self.0.iter() {
            total += s.raw_measure(purpose)?;
        }
        Ok(total)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let mut buf = u8::from(self.0.len() as u8).raw_encode(buf, purpose)?;

        for s in self.0.iter() {
            buf = s.raw_encode(buf, purpose)?;
        }

        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for StreamRanges {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let (count, buf) = u8::raw_decode(buf)?;
        let mut ranges = std::vec::Vec::new();
        let mut out_buff = buf;
        for _ in 0..count {
            let (range, buf) = StreamRange::raw_decode(out_buff)?;
            out_buff = buf;
            ranges.push(range);
        }

        Ok((Self(ranges), out_buff))
    }
}

#[test]
fn encode_protocol_stream_ranges() {
    use crate::interface::udp;

    let mut stream_ranges = Vec::new();
    stream_ranges.push(StreamRange {
        pos: rand::random::<u64>(),
        length: rand::random::<u32>(),
    });
    stream_ranges.push(StreamRange {
        pos: rand::random::<u64>(),
        length: rand::random::<u32>(),
    });

    let src = StreamRanges::from(stream_ranges);

    let mut buf = [0u8; udp::MTU];
    let remain = src.raw_encode(&mut buf, &None).unwrap();

    let remain = remain.len();
    let dec = &buf[..buf.len() - remain];
    let (dst, _) = StreamRanges::raw_decode(dec).unwrap();

    assert_eq!(dst.0.len(), src.0.len());

    let dst_hex = dst.to_hex().unwrap();
    let src_hex = dst.to_hex().unwrap();
    assert_eq!(dst_hex, src_hex);
}

pub const SESSIONDATA_FLAG_PACKAGEID: u16 = 1 << 0;
pub const SESSIONDATA_FLAG_ACK_PACKAGEID: u16 = 1 << 1;
pub const SESSIONDATA_FLAG_SYN: u16 = 1 << 2;
pub const SESSIONDATA_FLAG_ACK: u16 = 1 << 3;
pub const SESSIONDATA_FLAG_SACK: u16 = 1 << 4;
pub const SESSIONDATA_FLAG_SPEEDLIMIT: u16 = 1 << 5;
pub const SESSIONDATA_FLAG_SENDTIME: u16 = 1 << 6;
pub const SESSIONDATA_FLAG_PAYLOAD: u16 = 1 << 7;
pub const SESSIONDATA_FLAG_FIN: u16 = 1 << 10;
pub const SESSIONDATA_FLAG_FINACK: u16 = 1 << 11;
pub const SESSIONDATA_FLAG_RESET: u16 = 1 << 12;
pub const SESSIONDATA_FLAG_PING: u16 = 1 << 13;
pub const SESSIONDATA_FLAG_TO_SESSION_ID: u16 = 1 << 14;

pub struct SessionData {
    pub stream_pos: u64,
    pub ack_stream_pos: u64,
    pub sack: Option<StreamRanges>,
    pub session_id: IncreaseId,
    pub send_time: Timestamp,
    pub syn_info: Option<SessionSynInfo>,
    pub to_session_id: Option<IncreaseId>,
    pub id_part: Option<SessionDataPackageIdPart>,
    pub payload: TailedOwnedData,
    pub flags: u16,
}

impl std::fmt::Display for SessionData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut flags = String::from("");
        if self.is_flags_contain(SESSIONDATA_FLAG_PACKAGEID) {
            flags += "|PackageID";
        }
        if self.is_flags_contain(SESSIONDATA_FLAG_ACK_PACKAGEID) {
            flags += "|AckPackageID";
        }
        if self.is_flags_contain(SESSIONDATA_FLAG_SYN) {
            flags += "|Syn";
        }
        if self.is_flags_contain(SESSIONDATA_FLAG_ACK) {
            flags += "|Ack";
        }
        if self.is_flags_contain(SESSIONDATA_FLAG_FIN) {
            flags += "|Fin";
        }
        if self.is_flags_contain(SESSIONDATA_FLAG_FINACK) {
            flags += "|FinAck";
        }
        if self.is_flags_contain(SESSIONDATA_FLAG_RESET) {
            flags += "|Reset";
        }
        if self.is_flags_contain(SESSIONDATA_FLAG_PING) {
            flags += "|Ping";
        }

        let to_session_id = {
            if self.to_session_id.is_some() {
                format!("{}", self.to_session_id.clone().unwrap())
            } else {
                String::from("None")
            }
        };
        let syn_info = {
            if let Some(syn_info) = self.syn_info.as_ref() {
                format!(
                    ", syn info {{sequence:{:?}, from_session_id:{}, to_vport:{}}}",
                    syn_info.sequence, syn_info.from_session_id, syn_info.to_vport
                )
            } else {
                String::from("")
            }
        };
        let sack = {
            let mut ranges = Vec::new();
            if let Some(s) = &self.sack {
                for range in s.0.iter() {
                    let n = format!("[pos:{},len:{}]", range.pos, range.length);
                    //s += n;
                    ranges.push(n);
                }
            }
            let mut s = String::from("");
            for i in ranges.iter() {
                s += i;
            }
            s
        };
        write!(f, "SessionData {{pos:{},ack_pos:{},sack={},sendtime:{}, session_id:{},to_session_id:{}, payload:{}, flags:{} {}}}", self.stream_pos, self.ack_stream_pos,sack, self.send_time,self.session_id, to_session_id, self.payload.as_ref().len(), flags, syn_info)
    }
}

impl SessionData {
    pub fn new() -> Self {
        Self {
            stream_pos: 0,
            ack_stream_pos: 0,
            sack: None,
            session_id: IncreaseId::default(),
            send_time: 0,
            syn_info: None,
            to_session_id: None,
            payload: TailedOwnedData::from(Vec::new()),
            flags: 0,
            id_part: None,
        }
    }
    pub fn is_syn(&self) -> bool {
        self.syn_info.is_some() && (self.flags & SESSIONDATA_FLAG_ACK == 0)
    }

    pub fn is_syn_ack(&self) -> bool {
        self.syn_info.is_some() && (self.flags & SESSIONDATA_FLAG_ACK != 0)
    }

    pub fn set_ack(&mut self) -> &mut Self {
        self.flags = self.flags | SESSIONDATA_FLAG_ACK;
        self
    }

    pub fn is_flags_contain(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }

    pub fn flags_add(&mut self, flag: u16) {
        self.flags = self.flags | flag;
    }

    pub fn stream_pos_end(&self) -> u64 {
        self.stream_pos + self.payload.as_ref().len() as u64
    }

    pub fn encode_for_raw_data(&self, buf: &mut [u8]) -> BuckyResult<usize> {
        let buf_len = buf.len();
        let buf = (Self::cmd_code() as u8).raw_encode(buf, &None)?;
        let buf =
            self.raw_encode_with_context(buf, &mut merge_context::OtherEncode::default(), &None)?;
        Ok(buf_len - buf.len())
    }

    pub fn decode_from_raw_data(&self, buf: &[u8]) -> BuckyResult<Self> {
        let (pkg, _) =
            Self::raw_decode_with_context(buf, &mut merge_context::OtherDecode::default())?;
        Ok(pkg)
    }
    pub fn clone_with_data(&self) -> SessionData {
        let mut session = self.clone_without_data();

        let mut buf = vec![0; self.payload.as_ref().len()];
        buf.copy_from_slice(self.payload.as_ref());
        session.payload = TailedOwnedData::from(buf);

        session
    }
    
    pub fn clone_without_data(&self) -> SessionData {
        let mut session = SessionData::new();
        session.stream_pos = self.stream_pos;
        session.ack_stream_pos = self.ack_stream_pos;
        session.sack = match &self.sack {
            None => None,
            Some(v) => {
                let mut ranges = Vec::new();
                for item in v.0.iter() {
                    ranges.push(item.clone());
                }
                Some(StreamRanges(ranges))
            }
        };
        session.session_id = self.session_id.clone();
        session.send_time = self.send_time.clone();
        session.syn_info = self.syn_info.clone();
        session.to_session_id = self.to_session_id.clone();
        //TODO payload不进行数据copy，理论上是不会在有playload的情况下进行Clone的
        session.payload = TailedOwnedData::from(Vec::new());
        session.flags = self.flags.clone();
        session.id_part = self.id_part.clone();
        session
    }
}

impl Package for SessionData {
    fn version(&self) -> u8 {
        0
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::SessionData
    }
}

impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for SessionData {
    fn raw_measure_with_context(
        &self,
        _merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<usize, BuckyError> {
        unimplemented!()
    }

    fn raw_encode_with_context<'a>(
        &self,
        enc_buf: &'a mut [u8],
        merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        //log::debug!("==================================session={}", self);
        let (mut context, buf) = context::Encode::<Self, Context>::new(enc_buf, merge_context)?;
        let buf = context.encode(buf, &self.stream_pos, context::FLAG_ALWAYS_ENCODE)?;
        let buf = context.encode(buf, &self.ack_stream_pos, context::FLAG_ALWAYS_ENCODE)?;
        let buf = context.option_encode(buf, &self.sack, SESSIONDATA_FLAG_SACK)?;
        let buf = context.encode(buf, &self.session_id, context::FLAG_ALWAYS_ENCODE)?;
        let buf =
            context.check_encode(buf, "send_time", &self.send_time, SESSIONDATA_FLAG_SENDTIME)?;
        let buf = context.option_encode(buf, &self.syn_info, SESSIONDATA_FLAG_SYN)?;
        let buf =
            context.option_encode(buf, &self.to_session_id, SESSIONDATA_FLAG_TO_SESSION_ID)?;
        let id_flag = if self.is_flags_contain(SESSIONDATA_FLAG_PACKAGEID) {
            SESSIONDATA_FLAG_PACKAGEID
        } else {
            SESSIONDATA_FLAG_ACK_PACKAGEID
        };
        let buf = context.option_encode(buf, &self.id_part, id_flag)?;
        let _buf = context.encode(buf, &self.payload, context::FLAG_ALWAYS_ENCODE)?;
        context.set_flags(self.flags | context.get_flags());
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context> for SessionData {
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (stream_pos, buf) = context.decode(buf, "SessionData.stream_pos", context::FLAG_ALWAYS_DECODE)?;
        let (ack_stream_pos, buf) = context.decode(buf, "SessionData.ack_stream_pos", context::FLAG_ALWAYS_DECODE)?;
        let (sack, buf) = context.option_decode(buf, SESSIONDATA_FLAG_SACK)?;
        let (session_id, buf) = context.decode(buf, "SessionData.session_id", context::FLAG_ALWAYS_DECODE)?;
        let (send_time, buf) = context.check_decode(buf, "send_time", SESSIONDATA_FLAG_SENDTIME)?;
        let (syn_info, buf) = context.option_decode(buf, SESSIONDATA_FLAG_SYN)?;
        let (to_session_id, buf) = context.option_decode(buf, SESSIONDATA_FLAG_TO_SESSION_ID)?;
        let (id_part, buf) = context.option_decode(
            buf,
            SESSIONDATA_FLAG_PACKAGEID | SESSIONDATA_FLAG_ACK_PACKAGEID,
        )?;
        let (payload, buf) = context.decode(buf, "SessionData.payload", context::FLAG_ALWAYS_DECODE)?;

        Ok((
            Self {
                stream_pos,
                ack_stream_pos,
                sack,
                session_id,
                send_time,
                syn_info,
                to_session_id,
                payload,
                id_part,
                flags: context.flags(),
            },
            buf,
        ))
    }
}

// #[test]
// fn encode_protocol_session_data() {
//     use crate::interface::udp;

//     let data = "hello".as_bytes().to_vec();
//     let mut stream_ranges = Vec::new();
//     stream_ranges.push(StreamRange{
//         pos: rand::random::<u64>(),
//         length: rand::random::<u32>(),
//     });
//     stream_ranges.push(StreamRange{
//         pos: rand::random::<u64>(),
//         length: rand::random::<u32>(),
//     });

//     let src = SessionData {
//         stream_pos: rand::random::<u64>(),
//         ack_stream_pos: rand::random::<u64>(),
//         sack: None,
//         session_id: IncreaseId::default(),
//         send_time: bucky_time_now(),
//         syn_info: Some(SessionSynInfo{
//             sequence: TempSeq::from(rand::random::<u32>()),
//             from_session_id: IncreaseId::default(),
//             to_vport: rand::random::<u16>()
//         }),
//         to_session_id: Some(IncreaseId::default()),
//         id_part: Some(SessionDataPackageIdPart{
//             package_id: IncreaseId::default(),
//             total_recv: rand::random::<u64>()
//         }),
//         payload: TailedOwnedData::from(data),
//         flags: 0,
//     };

//     let mut buf = [0u8; udp::MTU];
//     let remain = src.raw_encode_with_context(
//         &mut buf,
//         &mut merge_context::OtherEncode::default(),
//         &None).unwrap();
//     let remain = remain.len();

//     let dec = &buf[..buf.len() - remain];
//     let (cmd, dec) = u8::raw_decode(dec).map(|(code, dec)| (PackageCmdCode::try_from(code).unwrap(), dec)).unwrap();
//     assert_eq!(cmd, PackageCmdCode::SessionData);
//     let (dst, _) = SessionData::raw_decode_with_context(&dec, &mut merge_context::OtherDecode::default()).unwrap();

//     assert_eq!(dst.stream_pos, src.stream_pos);
//     assert_eq!(dst.ack_stream_pos, src.ack_stream_pos);

//     let dst_sack = dst.sack.unwrap().to_hex().unwrap();
//     let src_sack = src.sack.unwrap().to_hex().unwrap();
//     assert_eq!(dst_sack, src_sack);

//     assert_eq!(dst.session_id, src.session_id);
//     assert_eq!(dst.send_time, src.send_time);

//     let dst_syn_info = dst.syn_info.unwrap().to_hex().unwrap();
//     let src_syn_info = src.syn_info.unwrap().to_hex().unwrap();
//     assert_eq!(dst_syn_info, src_syn_info);

//     assert_eq!(dst.to_session_id, src.to_session_id);

//     let dst_id_part = dst.id_part.unwrap().to_hex().unwrap();
//     let src_id_part = src.id_part.unwrap().to_hex().unwrap();
//     assert_eq!(dst_id_part, src_id_part);

//     let src_payload = src.payload.to_string();
//     let dst_payload = dst.payload.to_string();
//     assert_eq!(dst_payload, src_payload);

//     assert_eq!(dst.flags, src.flags);
// }

#[derive(Clone)]
pub struct TcpSynConnection {
    pub sequence: TempSeq,
    pub result: u8,
    pub to_vport: u16,
    pub from_session_id: IncreaseId,
    pub to_device_id: DeviceId,
    pub from_device_desc: Device,
    pub reverse_endpoint: Option<Vec<Endpoint>>,
    pub payload: TailedOwnedData,
}

impl std::fmt::Display for TcpSynConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TcpSynConnection:{{sequence:{:?},to_vport:{},from_device_id:{}, reverse_endpoint:{:?}}}",
            self.sequence, self.to_vport, self.from_device_desc.desc().device_id(), self.reverse_endpoint
        )
    }
}

impl Package for TcpSynConnection {
    fn version(&self) -> u8 {
        0
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::TcpSynConnection
    }
}


impl From<(&TcpSynConnection, Vec<u8>)> for Exchange {
    fn from(context: (&TcpSynConnection, Vec<u8>)) -> Self {
        let (tcp_syn, key_encrypted) = context;
	let mix_key_rand = AesKey::random();
        Exchange {
            sequence: tcp_syn.sequence.clone(),
            key_encrypted, 
            seq_key_sign: Signature::default(),
            send_time: bucky_time_now(),
            from_device_desc: tcp_syn.from_device_desc.clone(),
            mix_key: mix_key_rand,
        }
    }
}


impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for TcpSynConnection {
    fn raw_measure_with_context(
        &self,
        _merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<usize, BuckyError> {
        unimplemented!()
    }

    fn raw_encode_with_context<'a>(
        &self,
        enc_buf: &'a mut [u8],
        merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Encode::<Self, Context>::new(enc_buf, merge_context)?;
        let buf = context.check_encode(buf, "sequence", &self.sequence, flags.next())?;
        let buf = context.encode(buf, &self.result, flags.next())?;
        let buf = context.encode(buf, &self.to_vport, flags.next())?;
        let buf = context.encode(buf, &self.from_session_id, flags.next())?;
        let buf = context.check_encode(buf, "to_device_id", &self.to_device_id, flags.next())?;
        let buf = context.check_encode(buf, "device_desc", &self.from_device_desc, flags.next())?;
        let buf = context.option_encode(buf, &self.reverse_endpoint, flags.next())?;
        let _buf = context.encode(buf, &self.payload, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context>
    for TcpSynConnection
{
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (sequence, buf) = context.check_decode(buf, "sequence", flags.next())?;
        let (result, buf) = context.decode(buf, "TcpSynConnection.result", flags.next())?;
        let (to_vport, buf) = context.decode(buf, "TcpSynConnection.to_vport", flags.next())?;
        let (from_session_id, buf) = context.decode(buf, "TcpSynConnection.from_session_id", flags.next())?;
        let (to_device_id, buf) = context.check_decode(buf, "to_device_id", flags.next())?;
        let (from_device_desc, buf) = context.check_decode(buf, "device_desc", flags.next())?;
        let (reverse_endpoint, buf) = context.option_decode(buf, flags.next())?;
        let (payload, buf) = context.decode(buf, "TcpSynConnection.payload", flags.next())?;

        Ok((
            Self {
                sequence,
                result,
                to_vport,
                from_session_id,
                to_device_id,
                from_device_desc,
                reverse_endpoint,
                payload,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_tcp_syn_connection() {
    use crate::interface::udp;

    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let from_device = Device::new(
        None,
        UniqueId::default(),
        vec![],
        vec![],
        vec![],
        private_key.public(),
        Area::default(),
        DeviceCategory::PC,
    )
    .build();

    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let to_device = Device::new(
        None,
        UniqueId::default(),
        vec![],
        vec![],
        vec![],
        private_key.public(),
        Area::default(),
        DeviceCategory::PC,
    )
    .build();

    let data = "hello".as_bytes().to_vec();
    let mut eps = Vec::new();
    eps.push(Endpoint::from_str("W4tcp10.10.10.10:8060").unwrap());
    eps.push(Endpoint::from_str("W4tcp10.10.10.11:8060").unwrap());

    let src = TcpSynConnection {
        sequence: TempSeq::from(rand::random::<u32>()),
        result: rand::random::<u8>(),
        to_vport: rand::random::<u16>(),
        from_session_id: IncreaseId::default(),
        to_device_id: to_device.desc().device_id(),
        from_device_desc: from_device,
        reverse_endpoint: Some(eps),
        payload: TailedOwnedData::from(data),
    };

    let mut buf = [0u8; udp::MTU];
    let remain = src
        .raw_encode_with_context(&mut buf, &mut merge_context::OtherEncode::default(), &None)
        .unwrap();
    let remain = remain.len();

    let dec = &buf[..buf.len() - remain];
    let (cmd, dec) = u8::raw_decode(dec)
        .map(|(code, dec)| (PackageCmdCode::try_from(code).unwrap(), dec))
        .unwrap();
    assert_eq!(cmd, PackageCmdCode::TcpSynConnection);
    let (dst, _) =
        TcpSynConnection::raw_decode_with_context(&dec, &mut merge_context::OtherDecode::default())
            .unwrap();

    assert_eq!(dst.sequence, src.sequence);
    assert_eq!(dst.result, src.result);
    assert_eq!(dst.to_vport, src.to_vport);
    assert_eq!(dst.from_session_id, src.from_session_id);
    assert_eq!(dst.to_device_id, src.to_device_id);

    let dst_from_device_desc = dst.from_device_desc.to_hex().unwrap();
    let src_from_device_desc = src.from_device_desc.to_hex().unwrap();
    assert_eq!(dst_from_device_desc, src_from_device_desc);

    assert_eq!(dst.reverse_endpoint, src.reverse_endpoint);

    let dst_payload = dst.payload.to_string();
    let src_payload = src.payload.to_string();
    assert_eq!(dst_payload, src_payload);
}

pub const TCP_ACK_CONNECTION_RESULT_OK: u8 = 0;
pub const TCP_ACK_CONNECTION_RESULT_REFUSED: u8 = 1;

#[derive(Clone)]
pub struct TcpAckConnection {
    pub sequence: TempSeq,
    pub to_session_id: IncreaseId,
    pub result: u8,
    pub to_device_desc: Device,
    pub payload: TailedOwnedData,
}

impl std::fmt::Display for TcpAckConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TcpAckConnection:{{sequence:{:?},to_session_id:{}}}",
            self.sequence, self.to_session_id
        )
    }
}

impl Package for TcpAckConnection {
    fn version(&self) -> u8 {
        0
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::TcpAckConnection
    }
}

impl From<(&TcpAckConnection, Vec<u8>)> for Exchange {
    fn from(context: (&TcpAckConnection, Vec<u8>)) -> Self {
        let (tcp_ack, key_encrypted) = context;
	let mix_key_rand = AesKey::random();
        Exchange {
            sequence: tcp_ack.sequence.clone(),
            key_encrypted, 
            seq_key_sign: Signature::default(),
            send_time: bucky_time_now(),
            from_device_desc: tcp_ack.to_device_desc.clone(),
            mix_key: mix_key_rand,
        }
    }
}


impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for TcpAckConnection {
    fn raw_measure_with_context(
        &self,
        _merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<usize, BuckyError> {
        unimplemented!()
    }

    fn raw_encode_with_context<'a>(
        &self,
        enc_buf: &'a mut [u8],
        merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Encode::<Self, Context>::new(enc_buf, merge_context)?;
        let buf = context.check_encode(buf, "sequence", &self.sequence, flags.next())?;
        let buf = context.encode(buf, &self.to_session_id, flags.next())?;
        let buf = context.encode(buf, &self.result, flags.next())?;
        let buf = context.check_encode(buf, "device_desc", &self.to_device_desc, flags.next())?;
        let _buf = context.encode(buf, &self.payload, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context>
    for TcpAckConnection
{
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (sequence, buf) = context.check_decode(buf, "sequence", flags.next())?;
        let (to_session_id, buf) = context.decode(buf, "TcpAckConnection.to_session_id", flags.next())?;
        let (result, buf) = context.decode(buf, "TcpAckConnection.result", flags.next())?;
        let (to_device_desc, buf) = context.check_decode(buf, "device_desc", flags.next())?;
        let (payload, buf) = context.decode(buf, "TcpAckConnection.payload", flags.next())?;

        Ok((
            Self {
                sequence,
                result,
                to_session_id,
                to_device_desc,
                payload,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_tcp_ack_connection() {
    use crate::interface::udp;

    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let to_device = Device::new(
        None,
        UniqueId::default(),
        vec![],
        vec![],
        vec![],
        private_key.public(),
        Area::default(),
        DeviceCategory::PC,
    )
    .build();

    let data = "hello".as_bytes().to_vec();
    let src = TcpAckConnection {
        sequence: TempSeq::from(rand::random::<u32>()),
        to_session_id: IncreaseId::default(),
        result: rand::random::<u8>(),
        to_device_desc: to_device,
        payload: TailedOwnedData::from(data),
    };

    let mut buf = [0u8; udp::MTU];
    let remain = src
        .raw_encode_with_context(&mut buf, &mut merge_context::OtherEncode::default(), &None)
        .unwrap();
    let remain = remain.len();

    let dec = &buf[..buf.len() - remain];
    let (cmd, dec) = u8::raw_decode(dec)
        .map(|(code, dec)| (PackageCmdCode::try_from(code).unwrap(), dec))
        .unwrap();
    assert_eq!(cmd, PackageCmdCode::TcpAckConnection);
    let (dst, _) =
        TcpAckConnection::raw_decode_with_context(&dec, &mut merge_context::OtherDecode::default())
            .unwrap();

    assert_eq!(dst.sequence, src.sequence);
    assert_eq!(dst.to_session_id, src.to_session_id);
    assert_eq!(dst.result, src.result);

    let dst_device = dst.to_device_desc.to_hex().unwrap();
    let src_device = src.to_device_desc.to_hex().unwrap();
    assert_eq!(dst_device, src_device);

    let dst_payload = dst.payload.to_string();
    let src_payload = src.payload.to_string();
    assert_eq!(dst_payload, src_payload);
}

#[derive(Clone)]
pub struct TcpAckAckConnection {
    pub sequence: TempSeq,
    pub result: u8,
}

impl std::fmt::Display for TcpAckAckConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TcpAckAckConnection:{{sequence:{:?}}}", self.sequence)
    }
}

impl Package for TcpAckAckConnection {
    fn version(&self) -> u8 {
        0
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::TcpAckAckConnection
    }
}

impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for TcpAckAckConnection {
    fn raw_measure_with_context(
        &self,
        _merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<usize, BuckyError> {
        unimplemented!()
    }

    fn raw_encode_with_context<'a>(
        &self,
        enc_buf: &'a mut [u8],
        merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Encode::<Self, Context>::new(enc_buf, merge_context)?;
        let buf = context.check_encode(buf, "sequence", &self.sequence, flags.next())?;
        let _buf = context.encode(buf, &self.result, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context>
    for TcpAckAckConnection
{
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (sequence, buf) = context.check_decode(buf, "sequence", flags.next())?;
        let (result, buf) = context.decode(buf, "TcpAckAckConnection.result", flags.next())?;

        Ok((Self { sequence, result }, buf))
    }
}

#[test]
fn encode_protocol_tcp_ack_ack_connection() {
    use crate::interface::udp;

    let src = TcpAckAckConnection {
        sequence: TempSeq::from(rand::random::<u32>()),
        result: rand::random::<u8>(),
    };

    let mut buf = [0u8; udp::MTU];
    let remain = src
        .raw_encode_with_context(&mut buf, &mut merge_context::OtherEncode::default(), &None)
        .unwrap();
    let remain = remain.len();

    let dec = &buf[..buf.len() - remain];
    let (cmd, dec) = u8::raw_decode(dec)
        .map(|(code, dec)| (PackageCmdCode::try_from(code).unwrap(), dec))
        .unwrap();
    assert_eq!(cmd, PackageCmdCode::TcpAckAckConnection);
    let (dst, _) = TcpAckAckConnection::raw_decode_with_context(
        &dec,
        &mut merge_context::OtherDecode::default(),
    )
    .unwrap();

    assert_eq!(dst.sequence, src.sequence);
    assert_eq!(dst.result, src.result);
}




pub struct SnCallResp {
    //sn call的响应包
    pub seq: TempSeq,                 //序列事情
    pub sn_peer_id: DeviceId,         //sn设备id
    pub result: u8,                   //
    pub to_peer_info: Option<Device>, //
}

impl Package for SnCallResp {
    fn version(&self) -> u8 {
        0
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::SnCallResp
    }
}

impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for SnCallResp {
    fn raw_measure_with_context(
        &self,
        _merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<usize, BuckyError> {
        unimplemented!()
    }

    fn raw_encode_with_context<'a>(
        &self,
        enc_buf: &'a mut [u8],
        merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Encode::<Self, Context>::new(enc_buf, merge_context)?;
        let buf = context.check_encode(buf, "seq", &self.seq, flags.next())?;
        let buf = context.check_encode(buf, "sn_peer_id", &self.sn_peer_id, flags.next())?;
        let buf = context.encode(buf, &self.result, flags.next())?;
        let _buf =
            context.check_option_encode(buf, "to_peer_info", &self.to_peer_info, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context> for SnCallResp {
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (seq, buf) = context.check_decode(buf, "seq", flags.next())?;
        let (sn_peer_id, buf) = context.check_decode(buf, "sn_peer_id", flags.next())?;
        let (result, buf) = context.decode(buf, "SnCallResp.result", flags.next())?;
        let (to_peer_info, buf) = context.check_option_decode(buf, "to_peer_info", flags.next())?;

        Ok((
            Self {
                seq,
                sn_peer_id,
                result,
                to_peer_info,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_sn_call_resp() {
    use crate::interface::udp;

    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let to_device = Device::new(
        None,
        UniqueId::default(),
        vec![],
        vec![],
        vec![],
        private_key.public(),
        Area::default(),
        DeviceCategory::PC,
    )
    .build();

    let src = SnCallResp {
        seq: TempSeq::from(rand::random::<u32>()),
        sn_peer_id: to_device.desc().device_id(),
        result: rand::random::<u8>(),
        to_peer_info: Some(to_device),
    };

    let mut buf = [0u8; udp::MTU];
    let remain = src
        .raw_encode_with_context(&mut buf, &mut merge_context::OtherEncode::default(), &None)
        .unwrap();
    let remain = remain.len();

    let dec = &buf[..buf.len() - remain];
    let (cmd, dec) = u8::raw_decode(dec)
        .map(|(code, dec)| (PackageCmdCode::try_from(code).unwrap(), dec))
        .unwrap();
    assert_eq!(cmd, PackageCmdCode::SnCallResp);
    let (dst, _) =
        SnCallResp::raw_decode_with_context(&dec, &mut merge_context::OtherDecode::default())
            .unwrap();

    assert_eq!(dst.seq, src.seq);
    assert_eq!(dst.sn_peer_id, src.sn_peer_id);
    assert_eq!(dst.result, src.result);

    let dst_to_peer_info = dst.to_peer_info.to_hex().unwrap();
    let src_to_peer_info = src.to_peer_info.to_hex().unwrap();
    assert_eq!(dst_to_peer_info, src_to_peer_info);
}

#[derive(Clone)]
pub struct SnCalled {
    pub seq: TempSeq,
    pub sn_peer_id: DeviceId,
    pub to_peer_id: DeviceId,
    pub reverse_endpoint_array: Vec<Endpoint>,
    pub active_pn_list: Vec<DeviceId>,
    pub peer_info: Device,
    pub call_seq: TempSeq,
    pub call_send_time: Timestamp,
    pub payload: SizedOwnedData<SizeU16>,
}

impl Package for SnCalled {
    fn version(&self) -> u8 {
        0
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::SnCalled
    }
}

impl Into<merge_context::OtherDecode> for &SnCalled {
    fn into(self) -> merge_context::OtherDecode {
        let mut context = merge_context::FirstDecode::new();
        merge_context::Decode::set_name(&mut context, "sequence", &self.call_seq);
        merge_context::Decode::set_name(&mut context, "to_device_id", &self.to_peer_id);
        merge_context::Decode::set_name(&mut context, "send_time", &self.call_send_time);
        merge_context::Decode::set_name(&mut context, "device_desc", &self.peer_info);
        context.into()
    }
}

impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for SnCalled {
    fn raw_measure_with_context(
        &self,
        _merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<usize, BuckyError> {
        unimplemented!()
    }

    fn raw_encode_with_context<'a>(
        &self,
        enc_buf: &'a mut [u8],
        merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Encode::<Self, Context>::new(enc_buf, merge_context)?;
        let buf = context.encode(buf, &self.seq, flags.next())?;
        let buf = context.check_encode(buf, "sn_device_id", &self.sn_peer_id, flags.next())?;
        let buf = context.check_encode(buf, "to_device_id", &self.to_peer_id, flags.next())?;
        let buf = context.encode(buf, &self.reverse_endpoint_array, flags.next())?;
        let buf = context.encode(buf, &self.active_pn_list, flags.next())?;
        let buf = context.check_encode(buf, "device_desc", &self.peer_info, flags.next())?;
        let buf = context.check_encode(buf, "sequence", &self.call_seq, flags.next())?;
        let buf = context.check_encode(buf, "send_time", &self.call_send_time, flags.next())?;
        let _buf = context.encode(buf, &self.payload, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context> for SnCalled {
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (seq, buf) = context.check_decode(buf, "sequence", flags.next())?;
        let (sn_peer_id, buf) = context.check_decode(buf, "sn_device_id", flags.next())?;
        let (to_peer_id, buf) = context.check_decode(buf, "to_device_id", flags.next())?;
        let (reverse_endpoint_array, buf) = context.decode(buf, "SnCalled.reverse_endpoint_array", flags.next())?;
        let (active_pn_list, buf) = context.decode(buf, "SnCalled.active_pn_list", flags.next())?;
        let (peer_info, buf) = context.check_decode(buf, "device_desc", flags.next())?;
        let (call_seq, buf) = context.check_decode(buf, "sequence", flags.next())?;
        let (call_send_time, buf) = context.check_decode(buf, "send_time", flags.next())?;
        let (payload, buf) = context.decode(buf, "SnCalled.payload", flags.next())?;

        Ok((
            Self {
                seq,
                sn_peer_id,
                to_peer_id,
                reverse_endpoint_array,
                active_pn_list,
                peer_info,
                call_seq,
                call_send_time,
                payload,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_sn_called() {
    use crate::interface::udp;

    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let from_device = Device::new(
        None,
        UniqueId::default(),
        vec![],
        vec![],
        vec![],
        private_key.public(),
        Area::default(),
        DeviceCategory::PC,
    )
    .build();

    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let to_device = Device::new(
        None,
        UniqueId::default(),
        vec![],
        vec![],
        vec![],
        private_key.public(),
        Area::default(),
        DeviceCategory::PC,
    )
    .build();

    let data = "hello".as_bytes().to_vec();
    let mut eps = Vec::new();
    eps.push(Endpoint::from_str("W4tcp10.10.10.10:8060").unwrap());
    eps.push(Endpoint::from_str("W4tcp10.10.10.11:8060").unwrap());
    let mut pn = Vec::new();
    pn.push(from_device.desc().device_id().clone());

    let src = SnCalled {
        seq: TempSeq::from(rand::random::<u32>()),
        sn_peer_id: to_device.desc().device_id(),
        to_peer_id: to_device.desc().device_id(),
        reverse_endpoint_array: eps,
        active_pn_list: pn,
        peer_info: from_device,
        call_seq: TempSeq::from(rand::random::<u32>()),
        call_send_time: bucky_time_now(),
        payload: SizedOwnedData::from(data),
    };

    let mut buf = [0u8; udp::MTU];
    let remain = src
        .raw_encode_with_context(&mut buf, &mut merge_context::OtherEncode::default(), &None)
        .unwrap();
    let remain = remain.len();

    let dec = &buf[..buf.len() - remain];
    let (cmd, dec) = u8::raw_decode(dec)
        .map(|(code, dec)| (PackageCmdCode::try_from(code).unwrap(), dec))
        .unwrap();
    assert_eq!(cmd, PackageCmdCode::SnCalled);
    let (dst, _) =
        SnCalled::raw_decode_with_context(&dec, &mut merge_context::OtherDecode::default())
            .unwrap();

    assert_eq!(dst.seq, src.seq);
    assert_eq!(dst.sn_peer_id, src.sn_peer_id);
    assert_eq!(dst.to_peer_id, src.to_peer_id);
    assert_eq!(dst.reverse_endpoint_array, src.reverse_endpoint_array);

    let dst_pn = dst.active_pn_list.to_hex().unwrap();
    let src_pn = src.active_pn_list.to_hex().unwrap();
    assert_eq!(dst_pn, src_pn);

    let dst_peer_info = dst.peer_info.to_hex().unwrap();
    let src_peer_info = src.peer_info.to_hex().unwrap();
    assert_eq!(dst_peer_info, src_peer_info);

    assert_eq!(dst.call_seq, src.call_seq);
    assert_eq!(dst.call_send_time, src.call_send_time);

    let dst_payload = dst.payload.to_hex().unwrap();
    let src_payload = src.payload.to_hex().unwrap();
    assert_eq!(dst_payload, src_payload);
}

pub struct SnCalledResp {
    //sn called的应答报文
    pub seq: TempSeq,         //序列号
    pub sn_peer_id: DeviceId, //sn的设备id
    pub result: u8,           //
}

impl Package for SnCalledResp {
    fn version(&self) -> u8 {
        0
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::SnCalledResp
    }
}

impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for SnCalledResp {
    fn raw_measure_with_context(
        &self,
        _merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<usize, BuckyError> {
        unimplemented!()
    }

    fn raw_encode_with_context<'a>(
        &self,
        enc_buf: &'a mut [u8],
        merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Encode::<Self, Context>::new(enc_buf, merge_context)?;
        let buf = context.check_encode(buf, "seq", &self.seq, flags.next())?;
        let buf = context.check_encode(buf, "sn_peer_id", &self.sn_peer_id, flags.next())?;
        let _buf = context.check_encode(buf, "result", &self.result, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context> for SnCalledResp {
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (seq, buf) = context.check_decode(buf, "seq", flags.next())?;
        let (sn_peer_id, buf) = context.check_decode(buf, "sn_peer_id", flags.next())?;
        let (result, buf) = context.check_decode(buf, "result", flags.next())?;

        Ok((
            Self {
                seq,
                sn_peer_id,
                result,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_sn_called_resp() {
    use crate::interface::udp;

    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let to_device = Device::new(
        None,
        UniqueId::default(),
        vec![],
        vec![],
        vec![],
        private_key.public(),
        Area::default(),
        DeviceCategory::PC,
    )
    .build();

    let src = SnCalledResp {
        seq: TempSeq::from(rand::random::<u32>()),
        sn_peer_id: to_device.desc().device_id(),
        result: rand::random::<u8>(),
    };

    let mut buf = [0u8; udp::MTU];
    let remain = src
        .raw_encode_with_context(&mut buf, &mut merge_context::OtherEncode::default(), &None)
        .unwrap();
    let remain = remain.len();

    let dec = &buf[..buf.len() - remain];
    let (cmd, dec) = u8::raw_decode(dec)
        .map(|(code, dec)| (PackageCmdCode::try_from(code).unwrap(), dec))
        .unwrap();
    assert_eq!(cmd, PackageCmdCode::SnCalledResp);
    let (dst, _) =
        SnCalledResp::raw_decode_with_context(&dec, &mut merge_context::OtherDecode::default())
            .unwrap();

    assert_eq!(dst.seq, src.seq);
    assert_eq!(dst.sn_peer_id, src.sn_peer_id);
    assert_eq!(dst.result, src.result);
}




pub struct SnPingResp {
    //SN Server收到来自device的SNPing包时，返回device的外网地址
    pub seq: TempSeq,                      //包序列包
    pub sn_peer_id: DeviceId,              //sn的设备id
    pub result: u8,                        //是否接受device的接入
    pub peer_info: Option<Device>,         //sn的设备信息
    pub end_point_array: Vec<Endpoint>,    //外网地址列表
    pub receipt: Option<SnServiceReceipt>, //返回sn的一些连接信息，如当前连接的peer数量
}

impl Package for SnPingResp {
    fn version(&self) -> u8 {
        0
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::SnPingResp
    }
}

impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for SnPingResp {
    fn raw_measure_with_context(
        &self,
        _merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<usize, BuckyError> {
        unimplemented!()
    }

    fn raw_encode_with_context<'a>(
        &self,
        enc_buf: &'a mut [u8],
        merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Encode::<Self, Context>::new(enc_buf, merge_context)?;
        let buf = context.check_encode(buf, "seq", &self.seq, flags.next())?;
        let buf = context.check_encode(buf, "from_device_id", &self.sn_peer_id, flags.next())?;
        let buf = context.check_encode(buf, "result", &self.result, flags.next())?;
        let buf = context.check_option_encode(buf, "device_desc", &self.peer_info, flags.next())?;
        let buf = context.encode(buf, &self.end_point_array, flags.next())?;
        let _buf = context.option_encode(buf, &self.receipt, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context> for SnPingResp {
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (seq, buf) = context.check_decode(buf, "seq", flags.next())?;
        let (sn_peer_id, buf) = context.check_decode(buf, "from_device_id", flags.next())?;
        let (result, buf) = context.check_decode(buf, "result", flags.next())?;
        let (peer_info, buf) = context.check_option_decode(buf, "device_desc", flags.next())?;
        let (end_point_array, buf) = context.decode(buf, "SnPingResp.end_point_array", flags.next())?;
        let (receipt, buf) = context.option_decode(buf, flags.next())?;

        Ok((
            Self {
                seq,
                sn_peer_id,
                result,
                end_point_array,
                peer_info,
                receipt,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_sn_ping_resp() {
    use crate::interface::udp;

    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let from_device = Device::new(
        None,
        UniqueId::default(),
        vec![],
        vec![],
        vec![],
        private_key.public(),
        Area::default(),
        DeviceCategory::PC,
    )
    .build();

    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let to_device = Device::new(
        None,
        UniqueId::default(),
        vec![],
        vec![],
        vec![],
        private_key.public(),
        Area::default(),
        DeviceCategory::PC,
    )
    .build();

    let mut eps = Vec::new();
    eps.push(Endpoint::from_str("W4tcp10.10.10.10:8060").unwrap());
    eps.push(Endpoint::from_str("W4tcp10.10.10.11:8060").unwrap());
    let src = SnPingResp {
        seq: TempSeq::from(rand::random::<u32>()),
        sn_peer_id: to_device.desc().device_id(),
        result: rand::random::<u8>(),
        peer_info: Some(from_device),
        end_point_array: eps,
        receipt: Some(SnServiceReceipt {
            version: SnServiceReceiptVersion::Current,
            grade: SnServiceGrade::Normal,
            rto: rand::random::<u16>(),
            duration: std::time::Duration::from_secs(rand::random::<u8>() as u64),
            start_time: std::time::SystemTime::now(),
            ping_count: rand::random::<u32>(),
            ping_resp_count: rand::random::<u32>(),
            called_count: rand::random::<u32>(),
            call_peer_count: rand::random::<u32>(),
            connect_peer_count: rand::random::<u32>(),
            call_delay: rand::random::<u16>(),
        }),
    };

    let mut buf = [0u8; udp::MTU];
    let remain = src
        .raw_encode_with_context(&mut buf, &mut merge_context::OtherEncode::default(), &None)
        .unwrap();
    let remain = remain.len();

    let dec = &buf[..buf.len() - remain];
    let (cmd, dec) = u8::raw_decode(dec)
        .map(|(code, dec)| (PackageCmdCode::try_from(code).unwrap(), dec))
        .unwrap();
    assert_eq!(cmd, PackageCmdCode::SnPingResp);
    let (dst, _) =
        SnPingResp::raw_decode_with_context(&dec, &mut merge_context::OtherDecode::default())
            .unwrap();

    assert_eq!(dst.seq, src.seq);
    assert_eq!(dst.sn_peer_id, src.sn_peer_id);
    assert_eq!(dst.result, src.result);

    let dst_peer_info = dst.peer_info.to_hex().unwrap();
    let src_peer_info = src.peer_info.to_hex().unwrap();
    assert_eq!(dst_peer_info, src_peer_info);

    assert_eq!(dst.end_point_array, src.end_point_array);

    let dst_receipt = dst.receipt.to_hex().unwrap();
    let src_receipt = src.receipt.to_hex().unwrap();
    assert_eq!(dst_receipt, src_receipt);
}



#[derive(Debug)]
pub struct AckProxy {
    pub seq: TempSeq,
    pub to_peer_id: DeviceId,
    pub proxy_endpoint: Option<Endpoint>,
    pub err: Option<BuckyErrorCode>,
}

impl Package for AckProxy {
     fn version(&self) -> u8 {
        0
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::AckProxy
    }
}

impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for AckProxy {
    fn raw_measure_with_context(
        &self,
        _merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<usize, BuckyError> {
        unimplemented!()
    }

    fn raw_encode_with_context<'a>(
        &self,
        enc_buf: &'a mut [u8],
        merge_context: &mut Context,
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Encode::<Self, Context>::new(enc_buf, merge_context)?;
        let buf = context.encode(buf, &self.seq, flags.next())?;
        let buf = context.encode(buf, &self.to_peer_id, flags.next())?;
        let buf = context.option_encode(buf, &self.proxy_endpoint, flags.next())?;
        let _buf = context.option_encode(buf, &self.err, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context> for AckProxy {
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (seq, buf) = context.decode(buf, "AckProxy.seq", flags.next())?;
        let (to_peer_id, buf) = context.decode(buf, "AckProxy.to_peer_id", flags.next())?;
        let (proxy_endpoint, buf) = context.option_decode(buf, flags.next())?;
        let (err, buf) = context.option_decode(buf, flags.next())?;

        Ok((
            Self {
                seq,
                to_peer_id,
                proxy_endpoint,
                err,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_ack_proxy() {
    use crate::interface::udp;

    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let to_device = Device::new(
        None,
        UniqueId::default(),
        vec![],
        vec![],
        vec![],
        private_key.public(),
        Area::default(),
        DeviceCategory::PC,
    )
    .build();

    let src = AckProxy {
        seq: TempSeq::from(rand::random::<u32>()),
        to_peer_id: to_device.desc().device_id(),
        proxy_endpoint: Some(Endpoint::from_str("W4tcp10.10.10.10:8060").unwrap()),
        err: None,
    };

    let mut buf = [0u8; udp::MTU];
    let remain = src
        .raw_encode_with_context(&mut buf, &mut merge_context::OtherEncode::default(), &None)
        .unwrap();
    let remain = remain.len();

    let dec = &buf[..buf.len() - remain];
    let (cmd, dec) = u8::raw_decode(dec)
        .map(|(code, dec)| (PackageCmdCode::try_from(code).unwrap(), dec))
        .unwrap();
    assert_eq!(cmd, PackageCmdCode::AckProxy);
    let (dst, _) =
        AckProxy::raw_decode_with_context(&dec, &mut merge_context::OtherDecode::default())
            .unwrap();

    assert_eq!(dst.seq, src.seq);
    assert_eq!(dst.to_peer_id, src.to_peer_id);
    assert_eq!(dst.proxy_endpoint, src.proxy_endpoint);
}