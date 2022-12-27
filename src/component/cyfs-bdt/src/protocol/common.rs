mod dep {
    pub use std::any::Any;
    pub use std::collections::BTreeMap;
    pub use std::convert::TryFrom;
    pub use std::fmt;
    pub use std::rc::Rc;
    pub use std::str::FromStr;

    pub use cyfs_base::*;

    pub use crate::sn::types::*;
    pub use crate::types::*;
}

pub(super) use dep::*;


pub mod merge_context {
    use super::dep::*;

    pub trait Encode {
        fn fixed_value<T: RawMergable>(&self, name: &'static str) -> Option<Rc<T>>;
        // FIXME: no clone
        fn check_name<T: RawMergable>(&mut self, name: &'static str, value: &T) -> bool /*if merge*/;
    }
    pub trait Decode {
        fn check_name<T: Any + Clone>(&mut self, name: &'static str) -> Option<T>;
        fn set_name<T: Any + Clone>(&mut self, name: &'static str, value: &T);
    }
    pub type ContextNames = BTreeMap<&'static str, Rc<dyn Any>>;

    pub struct FixedValues(BTreeMap<&'static str, Rc<dyn Any>>);

    impl FixedValues {
        pub fn new() -> Self {
            Self(BTreeMap::new())
        }
        pub fn insert<T: 'static + RawEncode>(
            &mut self,
            name: &'static str,
            value: T,
        ) -> &mut Self {
            self.0.insert(name, Rc::new(value));
            self
        }

        pub fn get<T: RawMergable>(&self, name: &'static str) -> Option<Rc<T>> {
            self.0.get(name).map(|v| v.clone().downcast().unwrap())
        }

        pub fn clone_merged(&self) -> ContextNames {
            self.0.clone()
        }
    }

    // 编码第一个包的merge context
    pub struct FirstEncode<'a> {
        merged_names: ContextNames,
        fixed_values: Option<&'a FixedValues>,
    }

    impl FirstEncode<'_> {
        pub fn new() -> Self {
            Self {
                merged_names: ContextNames::new(),
                fixed_values: None,
            }
        }
    }

    impl<'a> From<&'a FixedValues> for FirstEncode<'a> {
        fn from(fixed_values: &'a FixedValues) -> Self {
            Self {
                merged_names: ContextNames::new(),
                fixed_values: Some(fixed_values),
            }
        }
    }

    impl<'a> Into<OtherEncode<'a>> for FirstEncode<'a> {
        fn into(self) -> OtherEncode<'a> {
            OtherEncode {
                merged_names: self.merged_names,
                fixed_values: self.fixed_values,
            }
        }
    }

    // 编码第一个包的时候，直接写入name
    impl Encode for FirstEncode<'_> {
        fn fixed_value<T: RawMergable>(&self, name: &'static str) -> Option<Rc<T>> {
            self.fixed_values.map_or(None, |v| v.get(name))
        }
        fn check_name<T: RawMergable>(&mut self, name: &'static str, value: &T) -> bool {
            self.merged_names.insert(name, Rc::new(value.clone()));
            false
        }
    }

    // 编码后续包的merge context
    pub struct OtherEncode<'a> {
        merged_names: ContextNames,
        fixed_values: Option<&'a FixedValues>,
    }

    impl<'a> Default for OtherEncode<'a> {
        fn default() -> Self {
            Self {
                merged_names: ContextNames::default(),
                fixed_values: None,
            }
        }
    }

    impl<'a> OtherEncode<'a> {
        pub fn new(
            merged_values: ContextNames,
            fixed_values: Option<&'a FixedValues>,
        ) -> OtherEncode<'a> {
            OtherEncode {
                merged_names: merged_values,
                fixed_values,
            }
        }
    }

    //编码后续包的时候，先检查名字是否存在，是否相等，如果相等就合并
    impl Encode for OtherEncode<'_> {
        fn fixed_value<T: RawMergable>(&self, name: &'static str) -> Option<Rc<T>> {
            self.fixed_values.map_or(None, |v| v.get(name))
        }
        fn check_name<T: RawMergable>(&mut self, name: &'static str, value: &T) -> bool {
            if let Some(to_merge) = self.merged_names.get(name) {
                value.raw_merge_ok(to_merge.as_ref().downcast_ref::<T>().unwrap())
            } else {
                false
            }
        }
    }

    // 解码第一个包的merge context
    pub struct FirstDecode {
        names: ContextNames, 
    }

    // 解码第一个包的时候，check name都返回None，set name写入value
    impl Decode for FirstDecode {
        fn check_name<T: Any + Clone>(&mut self, _name: &'static str) -> Option<T> {
            None
        }

        fn set_name<T: Any + Clone>(&mut self, name: &'static str, value: &T) {
            self.names.insert(name, Rc::new(value.clone()));
        }
    }

    impl FirstDecode {
        pub fn new() -> Self {
            Self {
                names: ContextNames::new(),
            }
        }
    }

    impl Into<OtherDecode> for FirstDecode {
        fn into(self) -> OtherDecode {
            OtherDecode::new(self.names)
        }
    }

    // 解码后续包的merge context
    pub struct OtherDecode {
        names: ContextNames, 
    }

    impl Default for OtherDecode {
        fn default() -> Self {
            Self {
                names: ContextNames::default(),
            }
        }
    }

    // 解码后续包的时候，set name什么都不干，check name检查第一个包设进来的name
    impl Decode for OtherDecode {
        fn check_name<T: Any + Clone>(&mut self, name: &str) -> Option<T> {
            if let Some(merged) = self.names.get(name) {
                Some(merged.as_ref().downcast_ref::<T>().unwrap().clone())
            } else {
                None
            }
        }

        fn set_name<T: Any + Clone>(&mut self, name: &'static str, value: &T) {
            self.names.insert(name, Rc::new(value.clone()));
        }
    }

    impl OtherDecode {
        pub fn new(names: ContextNames) -> Self {
            Self { names }
        }
    }
}

pub(super) mod context {
    use super::dep::*;
    use super::merge_context;
    use super::*;
    pub struct Encode<'enc, P: Package, MergeContext: merge_context::Encode> {
        flags: u16,
        length: usize,
        merge_context: &'enc mut MergeContext,
        reserved: std::marker::PhantomData<P>,
    }

    impl<'enc, P: Package, MergeContext: merge_context::Encode> Encode<'enc, P, MergeContext> {
        pub fn new<'a>(
            buf: &'a mut [u8],
            merge_context: &'enc mut MergeContext,
        ) -> Result<(Self, &'a mut [u8]), BuckyError> {
            let buf = (P::cmd_code() as u8).raw_encode(buf, &None)?;
            let buf = 0u16.raw_encode(buf, &None)?;
            Ok((
                Self {
                    flags: 0,
                    length: u8::raw_bytes().unwrap() + u16::raw_bytes().unwrap(),
                    merge_context,
                    reserved: std::marker::PhantomData::default(),
                },
                buf,
            ))
        }

        // 不检查是否merge
        pub fn encode<'a, T: RawEncode>(
            &mut self,
            buf: &'a mut [u8],
            value: &T,
            inc_flags: u16,
        ) -> Result<&'a mut [u8], BuckyError> {
            let pre_len = buf.len();
            self.flags |= inc_flags;
            let next_buf = value.raw_encode(buf, &None)?;
            self.length += pre_len - next_buf.len();
            Ok(next_buf)
        }

        pub fn option_encode<'a, T: RawEncode>(
            &mut self,
            buf: &'a mut [u8],
            value: &Option<T>,
            inc_flags: u16,
        ) -> Result<&'a mut [u8], BuckyError> {
            if let Some(v) = value {
                self.encode(buf, v, inc_flags)
            } else {
                Ok(buf)
            }
        }

        // 如果可以merge， 跳过value的编码
        pub fn check_encode<'a, T: RawEncode + RawMergable + Clone>(
            &mut self,
            buf: &'a mut [u8],
            name: &'static str,
            value: &T,
            inc_flags: u16,
        ) -> Result<&'a mut [u8], BuckyError> {
            let fixed = self.merge_context.fixed_value::<T>(name);
            let value = fixed.as_ref().map(|v| &**v).unwrap_or(value);
            let pre_len = buf.len();
            if self.merge_context.check_name(name, value) {
                Ok(buf)
            } else {
                self.flags |= inc_flags;
                let next_buf = value.raw_encode(buf, &None)?;
                self.length += pre_len - next_buf.len();
                Ok(next_buf)
            }
        }

        pub fn check_option_encode<'a, T: RawEncode + RawMergable + Clone>(
            &mut self,
            buf: &'a mut [u8],
            name: &'static str,
            value: &Option<T>,
            inc_flags: u16,
        ) -> Result<&'a mut [u8], BuckyError> {
            if let Some(v) = value {
                self.check_encode(buf, name, v, inc_flags)
            } else {
                Ok(buf)
            }
        }

        pub fn set_flags(&mut self, inc_flags: u16) {
            self.flags |= inc_flags;
        }

        pub fn get_flags(&self) -> u16 {
            self.flags
        }

        pub fn finish<'a>(&self, buf: &'a mut [u8]) -> Result<&'a mut [u8], BuckyError> {
            let begin_buf = buf;
            let buf = &mut begin_buf[u8::raw_bytes().unwrap()..];
            u16::from(self.flags).raw_encode(buf, &None).map(|_| ())?;
            Ok(&mut begin_buf[self.length..])
        }
    }

    pub struct Decode<'enc, MergeContext: merge_context::Decode> {
        flags: u16,
        merge_context: &'enc mut MergeContext,
    }

    impl<'enc, MergeContext: merge_context::Decode> Decode<'enc, MergeContext> {
        pub fn new<'a>(
            buf: &'a [u8],
            merge_context: &'enc mut MergeContext,
        ) -> Result<(Self, &'a [u8]), BuckyError> {
            let (flags, buf) = u16::raw_decode(buf)?;
            Ok((
                Self {
                    flags,
                    merge_context,
                },
                buf,
            ))
        }

        // 如果flags 的对应bit是0，使用merge context中的值
        pub fn check_decode<'a, T: 'static + RawDecode<'a> + Clone>(
            &mut self,
            buf: &'a [u8],
            name: &'static str,
            check_flags: u16,
        ) -> Result<(T, &'a [u8]), BuckyError> {
            let (opt, buf) = self.check_option_decode(buf, name, check_flags)?;
            Ok((
                opt.ok_or_else(|| BuckyError::new(BuckyErrorCode::InvalidData, format!("field {} no merged", name)))?,
                buf,
            ))
        }

        pub fn check_option_decode<'a, T: 'static + RawDecode<'a> + Clone>(
            &mut self,
            buf: &'a [u8],
            name: &'static str,
            check_flags: u16,
        ) -> Result<(Option<T>, &'a [u8]), BuckyError> {
            if self.flags & check_flags == 0 {
                let v = self.merge_context.check_name(name);
                Ok((v, buf))
            } else {
                let (v, buf) = T::raw_decode(buf)?;
                self.merge_context.set_name(name, &v);
                Ok((Some(v), buf))
            }
        }

        // 如果flags 的对应bit是0，会出错
        // TODO: 支持返回Option None
        pub fn decode<'a, T: RawDecode<'a>>(
            &mut self,
            buf: &'a [u8],
            name: &'static str, 
            check_flags: u16, 
        ) -> Result<(T, &'a [u8]), BuckyError> {
            if check_flags == FLAG_ALWAYS_DECODE {
                T::raw_decode(buf).map(|(v, buf)| (v, buf))
            } else {
                let (opt, buf) = self.option_decode(buf, check_flags)?;
                Ok((
                    opt.ok_or_else(|| BuckyError::new(BuckyErrorCode::InvalidData, format!("field {} no merged", name)))?,
                    buf,
                ))
            }
           
        }

        pub fn option_decode<'a, T: RawDecode<'a>>(
            &mut self,
            buf: &'a [u8],
            check_flags: u16,
        ) -> Result<(Option<T>, &'a [u8]), BuckyError> {
            if self.flags & check_flags == 0 {
                Ok((None, buf))
            } else {
                T::raw_decode(buf).map(|(v, buf)| (Some(v), buf))
            }
        }

        pub fn check_flags(&self, bits: u16) -> bool {
            self.flags & bits != 0
        }

        pub fn flags(&self) -> u16 {
            self.flags
        }
    }

    pub const FLAG_ALWAYS_ENCODE: u16 = 0;
    pub const FLAG_ALWAYS_DECODE: u16 = 0xffff;

    pub struct FlagsCounter {
        counter: u8,
    }

    impl FlagsCounter {
        pub fn new() -> Self {
            Self { counter: 0 }
        }

        pub fn next(&mut self) -> u16 {
            let inc = self.counter;
            self.counter += 1;
            1 << inc
        }
    }
}




#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd)]
pub enum PackageCmdCode {
    Exchange = 0,
    SynTunnel = 1,
    AckTunnel = 2,
    AckAckTunnel = 3,
    PingTunnel = 4,
    PingTunnelResp = 5,

    SnCall = 0x20,
    SnCallResp = 0x21,
    SnCalled = 0x22,
    SnCalledResp = 0x23,
    SnPing = 0x24,
    SnPingResp = 0x25,

    Datagram = 0x30,

    SessionData = 0x40,
    TcpSynConnection = 0x41,
    TcpAckConnection = 0x42,
    TcpAckAckConnection = 0x43,

    SynProxy = 0x50,
    AckProxy = 0x51,

    PieceData = 0x60,
    PieceControl = 0x61,
    ChannelEstimate = 0x62,
}

impl PackageCmdCode {
    pub fn is_exchange(&self) -> bool {
        *self == Self::Exchange
    }

    pub fn is_sn(&self) -> bool {
        (*self >= Self::SnCall) && (*self <= Self::SnPingResp)
    }

    pub fn is_tunnel(&self) -> bool {
        (*self >= Self::SynTunnel) && (*self <= Self::PingTunnelResp)
            || (*self >= Self::Datagram) && (*self <= Self::TcpSynConnection)
    }

    pub fn is_tcp_stream(&self) -> bool {
        (*self >= Self::TcpSynConnection) && (*self <= Self::TcpAckAckConnection)
    }

    pub fn is_session(&self) -> bool {
        (*self == Self::TcpSynConnection)
            || (*self == Self::SessionData)
            || (*self == Self::Datagram)
    }

    pub fn is_proxy(&self) -> bool {
        (*self >= Self::SynProxy) && (*self <= Self::AckProxy)
    }
}

impl TryFrom<u8> for PackageCmdCode {
    type Error = BuckyError;
    fn try_from(v: u8) -> std::result::Result<Self, Self::Error> {
        match v {
            0u8 => Ok(Self::Exchange),
            1u8 => Ok(Self::SynTunnel),
            2u8 => Ok(Self::AckTunnel),
            3u8 => Ok(Self::AckAckTunnel),
            4u8 => Ok(Self::PingTunnel),
            5u8 => Ok(Self::PingTunnelResp),
            0x20u8 => Ok(Self::SnCall),
            0x21u8 => Ok(Self::SnCallResp),
            0x22u8 => Ok(Self::SnCalled),
            0x23u8 => Ok(Self::SnCalledResp),
            0x24u8 => Ok(Self::SnPing),
            0x25u8 => Ok(Self::SnPingResp),
            0x30u8 => Ok(Self::Datagram),

            0x40u8 => Ok(Self::SessionData),
            0x41u8 => Ok(Self::TcpSynConnection),
            0x42u8 => Ok(Self::TcpAckConnection),
            0x43u8 => Ok(Self::TcpAckAckConnection),

            0x50u8 => Ok(Self::SynProxy),
            0x51u8 => Ok(Self::AckProxy),

            0x60u8 => Ok(Self::PieceData),
            0x61u8 => Ok(Self::PieceControl),
            0x62u8 => Ok(Self::ChannelEstimate),

            _ => Err(BuckyError::new(
                BuckyErrorCode::InvalidParam,
                "invalid package command type value",
            )),
        }
    }
}

pub trait Package {
    fn version(&self) -> u8;
    fn cmd_code() -> PackageCmdCode;
}

#[derive(Clone)]
pub struct Exchange {
    pub sequence: TempSeq,
    pub to_device_id: DeviceId, 
    pub send_time: Timestamp,
    pub key_encrypted: Vec<u8>, 
    pub sign: Signature,
    pub from_device_desc: Device,
    pub mix_key: AesKey,
}

impl std::fmt::Debug for Exchange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Exchange:{{sequence:{:?}, to_device_id:{:?}, from_device_desc:{}, mix_key:{:?}}}",
            self.sequence,
            self.to_device_id,
            self.from_device_desc.desc().device_id(), 
            self.mix_key.to_hex().unwrap(),
        )
    }
}

impl Exchange {
    pub async fn sign(&mut self, signer: &impl Signer) -> BuckyResult<()> {
        self.sign = signer
            .sign(
                self.to_sign().as_slice(),
                &SignatureSource::RefIndex(0),
            )
            .await?;
        Ok(())
    }

    pub async fn verify(&self, local: &DeviceId) -> bool {
        let verifier = RsaCPUObjectVerifier::new(self.from_device_desc.desc().public_key().clone());
        if verifier
            .verify(self.to_sign().as_slice(), &self.sign)
            .await {
            self.to_device_id.eq(local)
        } else {
            false
        }
    }

    fn to_sign(&self) -> HashValue {
        let seq = self.sequence.raw_encode_to_buffer().unwrap();
        let to_device_id = self.to_device_id.raw_encode_to_buffer().unwrap();
        let send_time = self.send_time.raw_encode_to_buffer().unwrap();
        use sha2::Digest;
        let mut sha256 = sha2::Sha256::new();
        sha256.input(&seq);
        sha256.input(&to_device_id);
        sha256.input(&send_time);
        sha256.input(&self.key_encrypted);
        sha256.result().into()
    }
}

impl Package for Exchange {
    fn version(&self) -> u8 {
        0
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::Exchange
    }
}


impl From<(&SynTunnel, Vec<u8>, AesKey)> for Exchange {
    fn from(context: (&SynTunnel, Vec<u8>, AesKey)) -> Self {
        let (syn_tunnel, key_encrypted, mix_key) = context;
        Exchange {
            sequence: syn_tunnel.sequence.clone(), 
            to_device_id: syn_tunnel.to_device_id.clone(), 
            send_time: syn_tunnel.send_time.clone(),
            key_encrypted, 
            sign: Signature::default(),
            from_device_desc: syn_tunnel.from_device_desc.clone(),
            mix_key
        }
    }
}


impl From<(&SynProxy, Vec<u8>, AesKey)> for Exchange {
    fn from(context: (&SynProxy, Vec<u8>, AesKey)) -> Self {
        let (syn_proxy, key_encrypted, mix_key) = context;
        Exchange {
            sequence: syn_proxy.seq, 
            to_device_id: syn_proxy.to_peer_id.clone(), 
            send_time: bucky_time_now(),
            key_encrypted, 
            sign: Signature::default(),
            from_device_desc: syn_proxy.from_peer_info.clone(),
            mix_key
        }
    }
}

impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for Exchange {
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
        let buf = context.check_encode(buf, "to_device_id", &self.to_device_id, flags.next())?;
        let buf = context.check_encode(buf, "send_time", &self.send_time, flags.next())?;
        let buf = context.encode(buf, &self.sign, flags.next())?;
        let buf =
            context.check_encode(buf, "device_desc", &self.from_device_desc, flags.next())?;
        let _buf =
            context.check_encode(buf, "mix_key", &self.mix_key, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context> for Exchange {
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (sequence, buf) = context.check_decode(buf, "sequence", flags.next())?;
        let (to_device_id, buf) = context.check_decode(buf, "to_device_id", flags.next())?;
        let (send_time, buf) = context.check_decode(buf, "send_time", flags.next())?;
        let (sign, buf) = context.decode(buf, "Exchange.seq_key_sign", flags.next())?;
        let (from_device_desc, buf) = context.check_decode(buf, "device_desc", flags.next())?;
        let (mix_key, buf) = context.check_decode(buf, "mix_key", flags.next())?;

        Ok((
            Self {
                sequence, 
                to_device_id, 
                send_time, 
                key_encrypted: vec![],
                sign,
                from_device_desc,
                mix_key
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_exchange() {
    use crate::interface::udp;

    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let device = Device::new(
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

    let (_key, key_encrypted) = private_key.public().gen_aeskey_and_encrypt().unwrap();
    let src = Exchange {
        sequence: TempSeq::from(rand::random::<u32>()), 
        to_device_id: DeviceId::default(), 
        send_time: bucky_time_now(),
        key_encrypted: key_encrypted.clone(), 
        sign: Signature::default(),
        from_device_desc: device,
        mix_key: AesKey::random(),
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
    assert_eq!(cmd, PackageCmdCode::Exchange);

    let mut dec_ctx = merge_context::OtherDecode::default();
    let (dst, _) =
        Exchange::raw_decode_with_context(dec, &mut dec_ctx).unwrap();

    assert_eq!(dst.sequence, src.sequence);
    assert_eq!(
        dst.from_device_desc.desc().device_id(),
        src.from_device_desc.desc().device_id()
    )
}


pub struct SynTunnel {
    pub protocol_version: u8,
    pub stack_version: u32,  
    pub to_device_id: DeviceId,
    pub sequence: TempSeq,
    pub from_device_desc: Device,
    pub send_time: Timestamp,
}


impl std::fmt::Debug for SynTunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SynTunnel:{{sequence:{:?}, to_device_id:{:?}, from_device_desc:{}}}",
            self.sequence,
            self.to_device_id,
            self.from_device_desc.desc().device_id(), 
        )
    }
}

impl Package for SynTunnel {
    fn version(&self) -> u8 {
        self.protocol_version
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::SynTunnel
    }
}

impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for SynTunnel {
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
        let buf = context.encode(buf, &self.protocol_version, context::FLAG_ALWAYS_ENCODE)?;
        let buf = context.encode(buf, &self.stack_version, context::FLAG_ALWAYS_ENCODE)?;
        let buf = context.check_encode(buf, "to_device_id", &self.to_device_id, flags.next())?;
        let buf = context.check_encode(buf, "sequence", &self.sequence, flags.next())?;
        let buf = context.check_encode(buf, "device_desc", &self.from_device_desc, flags.next())?;
        let _buf = context.check_encode(buf, "send_time", &self.send_time, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context> for SynTunnel {
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (protocol_version, buf) = context.decode(buf, "SynTunnel.protocol_version", context::FLAG_ALWAYS_DECODE)?;
        let (stack_version, buf) = context.decode(buf, "SynTunnel.stack_version", context::FLAG_ALWAYS_DECODE)?;
        let (to_device_id, buf) = context.check_decode(buf, "to_device_id", flags.next())?;
        let (sequence, buf) = context.check_decode(buf, "sequence", flags.next())?;
        let (from_device_desc, buf) = context.check_decode(buf, "device_desc", flags.next())?;
        let (send_time, buf) = context.check_decode(buf, "send_time", flags.next())?;
        Ok((
            Self {
                protocol_version, 
                stack_version, 
                to_device_id,
                sequence,
                from_device_desc,
                send_time,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_syn_tunnel() {
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

    let src = SynTunnel {
        protocol_version: 0, 
        stack_version: 0,  
        to_device_id: to_device.desc().device_id(),
        sequence: TempSeq::from(rand::random::<u32>()),
        from_device_desc: from_device,
        send_time: bucky_time_now(),
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
    assert_eq!(cmd, PackageCmdCode::SynTunnel);
    let (dst, _) =
        SynTunnel::raw_decode_with_context(&dec, &mut merge_context::OtherDecode::default())
            .unwrap();

    assert_eq!(dst.sequence, src.sequence);
    assert_eq!(dst.to_device_id, src.to_device_id);
    assert_eq!(
        dst.from_device_desc.desc().device_id(),
        src.from_device_desc.desc().device_id()
    );
}

pub const ACK_TUNNEL_RESULT_OK: u8 = 0;
pub const ACK_TUNNEL_RESULT_REFUSED: u8 = 1;

#[derive(Debug)]
pub struct AckTunnel {
    pub protocol_version: u8, 
    pub stack_version: u32, 
    pub sequence: TempSeq,
    pub result: u8,
    pub send_time: Timestamp,
    pub mtu: u16,
    pub to_device_desc: Device,
}

impl Package for AckTunnel {
    fn version(&self) -> u8 {
        self.protocol_version
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::AckTunnel
    }
}

impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for AckTunnel {
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
        let buf = context.encode(buf, &self.protocol_version, context::FLAG_ALWAYS_ENCODE)?;
        let buf = context.encode(buf, &self.stack_version, context::FLAG_ALWAYS_ENCODE)?;
        let buf = context.check_encode(buf, "sequence", &self.sequence, flags.next())?;
        let buf = context.encode(buf, &self.result, flags.next())?;
        let buf = context.check_encode(buf, "send_time", &self.send_time, flags.next())?;
        let buf = context.encode(buf, &self.mtu, flags.next())?;
        let _buf = context.check_encode(buf, "device_desc", &self.to_device_desc, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context> for AckTunnel {
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (protocol_version, buf) = context.decode(buf, "AckTunnel.protocol_version", context::FLAG_ALWAYS_DECODE)?;
        let (stack_version, buf) = context.decode(buf, "AckTunnel.stack_version", context::FLAG_ALWAYS_DECODE)?;
        let (sequence, buf) = context.check_decode(buf, "sequence", flags.next())?;
        let (result, buf) = context.decode(buf, "AckTunnel.result", flags.next())?;
        let (send_time, buf) = context.check_decode(buf, "send_time", flags.next())?;
        let (mtu, buf) = context.decode(buf, "AckTunnel.mtu", flags.next())?;
        let (to_device_desc, buf) = context.check_decode(buf, "device_desc", flags.next())?;
        Ok((
            Self {
                protocol_version, 
                stack_version, 
                sequence,
                result,
                send_time,
                mtu,
                to_device_desc,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_ack_tunnel() {
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

    let src = AckTunnel {
        protocol_version: 0, 
        stack_version: 0, 
        sequence: TempSeq::from(rand::random::<u32>()),
        result: rand::random::<u8>(),
        send_time: bucky_time_now(),
        mtu: rand::random::<u16>(),
        to_device_desc: to_device,
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
    assert_eq!(cmd, PackageCmdCode::AckTunnel);
    let (dst, _) =
        AckTunnel::raw_decode_with_context(&dec, &mut merge_context::OtherDecode::default())
            .unwrap();

    assert_eq!(dst.sequence, src.sequence);
    assert_eq!(dst.result, src.result);
    assert_eq!(dst.send_time, src.send_time);
    assert_eq!(dst.mtu, src.mtu);
    assert_eq!(
        dst.to_device_desc.desc().device_id(),
        src.to_device_desc.desc().device_id()
    )
}



#[derive(Clone)]
pub struct SnCall {
    pub protocol_version: u8, 
    pub stack_version: u32, 
    pub seq: TempSeq,
    pub sn_peer_id: DeviceId,
    pub to_peer_id: DeviceId,
    pub from_peer_id: DeviceId,
    pub reverse_endpoint_array: Option<Vec<Endpoint>>,
    pub active_pn_list: Option<Vec<DeviceId>>,
    pub peer_info: Option<Device>,
    pub send_time: Timestamp,
    pub payload: SizedOwnedData<SizeU16>,
    pub is_always_call: bool,
}


impl std::fmt::Debug for SnCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SnCall:{{seq:{:?}, sn_peer_id:{:?}, to_peer_id:{}, from_peer_id:{:?}, reverse_endpoint_array:{:?}, active_pn_list:{:?}, peer_info:{}, payload:{}}}",
            self.seq,
            self.sn_peer_id,
            self.to_peer_id, 
            self.from_peer_id,
            self.reverse_endpoint_array, 
            self.active_pn_list, 
            self.peer_info.is_some(),
            self.payload.len()
        )
    }
}


impl Package for SnCall {
    fn version(&self) -> u8 {
        self.protocol_version
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::SnCall
    }
}



impl From<(&SnCall, Vec<u8>, AesKey)> for Exchange {
    fn from(context: (&SnCall, Vec<u8>, AesKey)) -> Self {
        let (sn_call, key_encrypted, mix_key) = context;
    
        Self {
            sequence: sn_call.seq.clone(),  
            to_device_id: sn_call.sn_peer_id.clone(), 
            send_time: sn_call.send_time,  
            key_encrypted, 
            sign: Signature::default(),
            from_device_desc: sn_call.peer_info.clone().unwrap(),
            mix_key
        }
    }
}

impl Into<merge_context::FixedValues> for &SnCall {
    fn into(self) -> merge_context::FixedValues {
        let mut v = merge_context::FixedValues::new();
        v.insert("sequence", self.seq)
            .insert("to_device_id", self.to_peer_id.clone())
            .insert("from_device_id", self.from_peer_id.clone())
            .insert("send_time", self.send_time);
        if let Some(dev) = self.peer_info.as_ref() {
            v.insert("device_desc", dev.clone());
        }
        v
    }
}

impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for SnCall {
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
        let buf = context.encode(buf, &self.protocol_version, context::FLAG_ALWAYS_ENCODE)?;
        let buf = context.encode(buf, &self.stack_version, context::FLAG_ALWAYS_ENCODE)?;
        let buf = context.check_encode(buf, "sequence", &self.seq, flags.next())?;
        let buf = context.check_encode(buf, "sn_device_id", &self.sn_peer_id, flags.next())?;
        let buf = context.check_encode(buf, "to_device_id", &self.to_peer_id, flags.next())?;
        let buf = context.check_encode(buf, "from_device_id", &self.from_peer_id, flags.next())?;
        let buf = context.encode(buf, &self.reverse_endpoint_array, flags.next())?;
        let buf = context.encode(buf, &self.active_pn_list, flags.next())?;
        let buf = context.check_option_encode(buf, "device_desc", &self.peer_info, flags.next())?;
        let buf = context.check_encode(buf, "send_time", &self.send_time, flags.next())?;
        let _buf = context.encode(buf, &self.payload, flags.next())?;
        context.set_flags({
            let f = flags.next();
            if self.is_always_call {
                f
            } else {
                0
            }
        });
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context> for SnCall {
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (protocol_version, buf) = context.decode(buf, "SnCall.protocol_version", context::FLAG_ALWAYS_DECODE)?;
        let (stack_version, buf) = context.decode(buf, "SnCall.stack_version", context::FLAG_ALWAYS_DECODE)?;
        let (seq, buf) = context.check_decode(buf, "sequence", flags.next())?;
        let (sn_peer_id, buf) = context.check_decode(buf, "sn_device_id", flags.next())?;
        let (to_peer_id, buf) = context.check_decode(buf, "to_device_id", flags.next())?;
        let (from_peer_id, buf) = context.check_decode(buf, "from_device_id", flags.next())?;
        let (reverse_endpoint_array, buf) = context.decode(buf, "SnCall.reverse_endpoint_array", flags.next())?;
        let (active_pn_list, buf) = context.decode(buf, "SnCall.active_pn_list", flags.next())?;
        let (peer_info, buf) = context.check_option_decode(buf, "device_desc", flags.next())?;
        let (send_time, buf) = context.check_decode(buf, "send_time", flags.next())?;
        let (payload, buf) = context.decode(buf, "SnCall.payload", flags.next())?;
        let is_always_call = context.check_flags(flags.next());

        Ok((
            Self {
                protocol_version, 
                stack_version, 
                seq,
                to_peer_id,
                from_peer_id,
                sn_peer_id,
                reverse_endpoint_array,
                active_pn_list,
                peer_info,
                payload,
                send_time,
                is_always_call,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_sn_call() {
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

    let src = SnCall {
        protocol_version: 0,
        stack_version: 0, 
        seq: TempSeq::from(rand::random::<u32>()),
        sn_peer_id: to_device.desc().device_id(),
        to_peer_id: to_device.desc().device_id(),
        from_peer_id: from_device.desc().device_id(),
        reverse_endpoint_array: Some(eps),
        active_pn_list: Some(pn),
        peer_info: Some(from_device),
        send_time: bucky_time_now(),
        payload: SizedOwnedData::from(data),
        is_always_call: true,
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
    assert_eq!(cmd, PackageCmdCode::SnCall);
    let (dst, _) =
        SnCall::raw_decode_with_context(&dec, &mut merge_context::OtherDecode::default()).unwrap();

    assert_eq!(dst.seq, src.seq);
    assert_eq!(dst.sn_peer_id, src.sn_peer_id);
    assert_eq!(dst.to_peer_id, src.to_peer_id);
    assert_eq!(dst.from_peer_id, src.from_peer_id);
    assert_eq!(dst.reverse_endpoint_array, src.reverse_endpoint_array);

    let dst_pn = dst.active_pn_list.to_hex().unwrap();
    let src_pn = src.active_pn_list.to_hex().unwrap();
    assert_eq!(dst_pn, src_pn);

    let dst_peer_info = dst.peer_info.to_hex().unwrap();
    let src_peer_info = src.peer_info.to_hex().unwrap();
    assert_eq!(dst_peer_info, src_peer_info);

    assert_eq!(dst.send_time, src.send_time);

    let dst_payload = dst.payload.to_hex().unwrap();
    let src_payload = src.payload.to_hex().unwrap();
    assert_eq!(dst_payload, src_payload);

    assert_eq!(dst.is_always_call, src.is_always_call);
}

#[derive(Debug)]
pub struct SnPing {
    pub protocol_version: u8, 
    pub stack_version: u32, 
    //ln与sn的keepalive包
    pub seq: TempSeq,                          //序列号
    pub sn_peer_id: DeviceId,                  //sn的设备id
    pub from_peer_id: Option<DeviceId>,        //发送者设备id
    pub peer_info: Option<Device>,             //发送者设备信息
    pub send_time: Timestamp,                  //发送时间
    pub contract_id: Option<ObjectId>,         //合约文件对象id
    pub receipt: Option<ReceiptWithSignature>, //客户端提供的服务清单
}

impl From<(&SnPing, Device, Vec<u8>, AesKey)> for Exchange {
    fn from(context: (&SnPing, Device, Vec<u8>, AesKey)) -> Self {
        let (sn_ping, local_device, key_encrypted, mix_key) = context;
        Self {
            sequence: sn_ping.seq.clone(), 
            to_device_id: sn_ping.sn_peer_id.clone(), 
            send_time: sn_ping.send_time,
            key_encrypted,
            sign: Signature::default(),
            from_device_desc: local_device, 
            mix_key, 
        }
    }
}


impl Into<merge_context::FixedValues> for &SnPing {
    fn into(self) -> merge_context::FixedValues {
        let mut v = merge_context::FixedValues::new();
        v.insert("sequence", self.seq)
            .insert("to_device_id", self.sn_peer_id.clone())
            .insert("from_device_id", self.from_peer_id.clone())
            .insert("device_desc", self.peer_info.clone())
            .insert("send_time", self.send_time);
        v
    }
}

impl Package for SnPing {
    fn version(&self) -> u8 {
        self.protocol_version
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::SnPing
    }
}

impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for SnPing {
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
        let buf = context.encode(buf, &self.protocol_version, context::FLAG_ALWAYS_ENCODE)?;
        let buf = context.encode(buf, &self.stack_version, context::FLAG_ALWAYS_ENCODE)?;
        let buf = context.check_encode(buf, "seq", &self.seq, flags.next())?;
        let buf = context.check_encode(buf, "to_device_id", &self.sn_peer_id, flags.next())?;
        let buf =
            context.check_option_encode(buf, "from_device_id", &self.from_peer_id, flags.next())?;
        let buf = context.check_option_encode(buf, "device_desc", &self.peer_info, flags.next())?;
        let buf = context.check_encode(buf, "send_time", &self.send_time, flags.next())?;
        let buf = context.option_encode(buf, &self.contract_id, flags.next())?;
        let _buf = context.option_encode(buf, &self.receipt, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context> for SnPing {
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (protocol_version, buf) = context.decode(buf, "SnPing.protocol_version", context::FLAG_ALWAYS_DECODE)?;
        let (stack_version, buf) = context.decode(buf, "SnPing.stack_version", context::FLAG_ALWAYS_DECODE)?;
        let (seq, buf) = context.check_decode(buf, "seq", flags.next())?;
        let (sn_peer_id, buf) = context.check_decode(buf, "to_device_id", flags.next())?;
        let (from_peer_id, buf) =
            context.check_option_decode(buf, "from_device_id", flags.next())?;
        let (peer_info, buf) = context.check_option_decode(buf, "device_desc", flags.next())?;
        let (send_time, buf) = context.check_decode(buf, "send_time", flags.next())?;
        let (contract_id, buf) = context.option_decode(buf, flags.next())?;
        let (receipt, buf) = context.option_decode(buf, flags.next())?;

        Ok((
            Self {
                protocol_version, 
                stack_version, 
                seq,
                from_peer_id,
                sn_peer_id,
                peer_info,
                send_time,
                contract_id,
                receipt,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_sn_ping() {
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

    let ssr = SnServiceReceipt {
        version: SnServiceReceiptVersion::Invalid,
        grade: SnServiceGrade::None,
        rto: rand::random::<u16>(),
        duration: std::time::Duration::from_millis(0),
        start_time: std::time::UNIX_EPOCH,
        ping_count: rand::random::<u32>(),
        ping_resp_count: rand::random::<u32>(),
        called_count: rand::random::<u32>(),
        call_peer_count: rand::random::<u32>(),
        connect_peer_count: rand::random::<u32>(),
        call_delay: rand::random::<u16>(),
    };

    let sig = Signature::default();
    let receipt = ReceiptWithSignature::from((ssr, sig));
    let src = SnPing {
        protocol_version: 0, 
        stack_version: 0, 
        seq: TempSeq::from(rand::random::<u32>()),
        sn_peer_id: to_device.desc().device_id(),
        from_peer_id: Some(from_device.desc().device_id()),
        peer_info: Some(to_device),
        send_time: bucky_time_now(),
        contract_id: Some(
            ObjectId::from_str("5aSixgLtjoYcAFH9isc6KCqDgKfTJ8jpgASAoiRz5NLk").unwrap(),
        ),
        receipt: Some(receipt),
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
    assert_eq!(cmd, PackageCmdCode::SnPing);
    let (dst, _) =
        SnPing::raw_decode_with_context(&dec, &mut merge_context::OtherDecode::default()).unwrap();

    assert_eq!(dst.seq, src.seq);
    assert_eq!(dst.sn_peer_id, src.sn_peer_id);
    assert_eq!(dst.from_peer_id, src.from_peer_id);

    let dst_peer_info = dst.peer_info.to_hex().unwrap();
    let src_peer_info = src.peer_info.to_hex().unwrap();
    assert_eq!(dst_peer_info, src_peer_info);

    assert_eq!(dst.send_time, src.send_time);

    let dst_contract_id = dst.contract_id.to_hex().unwrap();
    let src_contract_id = src.contract_id.to_hex().unwrap();
    assert_eq!(dst_contract_id, src_contract_id);

    let dst_receipt = dst.receipt.unwrap().to_hex().unwrap();
    let src_receipt = src.receipt.unwrap().to_hex().unwrap();
    assert_eq!(dst_receipt, src_receipt);
}



#[derive(Clone)]
pub struct SynProxy {
    pub protocol_version: u8, 
    pub stack_version: u32, 
    pub seq: TempSeq,
    pub to_peer_id: DeviceId,
    pub to_peer_timestamp: Timestamp,
    pub from_peer_info: Device,
    pub mix_key: AesKey,
}

impl std::fmt::Display for SynProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SynProxy:{{sequence:{:?}, to:{:?}, from:{}, mix_key:{:?}}}",
            self.seq,
            self.to_peer_id,
            self.from_peer_info.desc().device_id(), 
            self.mix_key.to_hex(),
        )
    }
}

impl std::fmt::Debug for SynProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SynProxy:{{sequence:{:?}, to:{:?}, from:{}, mix_key:{:?}}}",
            self.seq,
            self.to_peer_id,
            self.from_peer_info.desc().device_id(), 
            self.mix_key.to_hex(),
        )
    }
}

impl Package for SynProxy {
    fn version(&self) -> u8 {
        self.protocol_version
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::SynProxy
    }
}

impl<Context: merge_context::Encode> RawEncodeWithContext<Context> for SynProxy {
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
        let buf = context.encode(buf, &self.protocol_version, context::FLAG_ALWAYS_ENCODE)?;
        let buf = context.encode(buf, &self.stack_version, context::FLAG_ALWAYS_ENCODE)?;
        let buf = context.check_encode(buf, "seq", &self.seq, flags.next())?;
        let buf = context.check_encode(buf, "to_device_id", &self.to_peer_id, flags.next())?;
        let buf = context.encode(buf, &self.to_peer_timestamp, flags.next())?;
        let buf = context.check_encode(buf, "device_desc", &self.from_peer_info, flags.next())?;
        let _buf = context.encode(buf, &self.mix_key, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context> for SynProxy {
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let mut flags = context::FlagsCounter::new();
        let (mut context, buf) = context::Decode::new(buf, merge_context)?;
        let (protocol_version, buf) = context.decode(buf, "SynProxy.protocol_version", context::FLAG_ALWAYS_DECODE)?;
        let (stack_version, buf) = context.decode(buf, "SynProxy.stack_version", context::FLAG_ALWAYS_DECODE)?;
        let (seq, buf) = context.check_decode(buf, "seq", flags.next())?;
        let (to_peer_id, buf) = context.check_decode(buf, "to_device_id", flags.next())?;
        let (to_peer_timestamp, buf) = context.decode(buf, "SynProxy.to_peer_timestamp", flags.next())?;
        let (from_peer_info, buf) = context.check_decode(buf, "device_desc", flags.next())?;
        let (mix_key, buf) = context.decode(buf, "SynProxy.mix_key", flags.next())?;

        Ok((
            Self {
                protocol_version, 
                stack_version, 
                seq,
                to_peer_id,
                to_peer_timestamp,
                from_peer_info,
                mix_key,
            },
            buf,
        ))
    }
}

#[test]
fn encode_protocol_syn_proxy() {
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

    let src = SynProxy {
        protocol_version: 0,
        stack_version: 0, 
        seq: TempSeq::from(rand::random::<u32>()),
        to_peer_id: to_device.desc().device_id(),
        to_peer_timestamp: bucky_time_now(),
        from_peer_info: from_device,
        mix_key: AesKey::random(),
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
    assert_eq!(cmd, PackageCmdCode::SynProxy);
    let (dst, _) =
        SynProxy::raw_decode_with_context(&dec, &mut merge_context::OtherDecode::default())
            .unwrap();

    assert_eq!(dst.seq, src.seq);
    assert_eq!(dst.to_peer_id, src.to_peer_id);
    assert_eq!(dst.to_peer_timestamp, src.to_peer_timestamp);

    let dst_peer_info = dst.from_peer_info.to_hex().unwrap();
    let src_peer_info = src.from_peer_info.to_hex().unwrap();
    assert_eq!(dst_peer_info, src_peer_info);
}
