mod dep {
    pub use std::any::Any;
    pub use std::collections::BTreeMap;
    pub use std::convert::TryFrom;
    pub use std::fmt;
    pub use std::rc::Rc;
    pub use std::str::FromStr;
    pub use std::mem;

    pub use cyfs_base::*;

    pub use crate::sn::types::*;
    pub use crate::types::*;
}

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

mod context {
    use super::dep::*;
    use super::merge_context;
    use super::*;
    pub struct Encode<'enc, P: Package, MergeContext: merge_context::Encode> {
        flags: u16,
        pub length: usize,
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
                opt.ok_or_else(|| BuckyError::new(BuckyErrorCode::InvalidData, "no merged"))?,
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
            check_flags: u16,
        ) -> Result<(T, &'a [u8]), BuckyError> {
            let (opt, buf) = self.option_decode(buf, check_flags)?;
            Ok((
                opt.ok_or_else(|| BuckyError::new(BuckyErrorCode::InvalidData, "no merged"))?,
                buf,
            ))
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

use dep::*;

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
                format!("invalid package command type value {}", v),
            )),
        }
    }
}

pub trait Package {
    fn cmd_code() -> PackageCmdCode;
}

#[derive(Clone)]
pub struct Exchange {
    pub sequence: TempSeq,
    pub seq_key_sign: Signature,
    pub from_device_id: DeviceId,
    pub send_time: Timestamp,
    pub from_device_desc: Device,
}

impl Exchange {
    pub async fn sign(&mut self, key: &AesKey, signer: &impl Signer) -> BuckyResult<()> {
        self.seq_key_sign = signer
            .sign(
                self.seq_key_hash(key).as_slice(),
                &SignatureSource::RefIndex(0),
            )
            .await?;
        Ok(())
    }

    pub async fn verify(&self, key: &AesKey) -> bool {
        let verifier = RsaCPUObjectVerifier::new(self.from_device_desc.desc().public_key().clone());
        verifier
            .verify(self.seq_key_hash(key).as_slice(), &self.seq_key_sign)
            .await
    }

    fn seq_key_hash(&self, key: &AesKey) -> HashValue {
        let mut buf = [0u8; 128];
        let len = buf.len();
        let remain = self.sequence.raw_encode(&mut buf, &None).unwrap();
        let remain = key.raw_encode(remain, &None).unwrap();
        let len = len - remain.len();
        hash_data(&buf[..len])
    }
}

impl Package for Exchange {
    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::Exchange
    }
}

impl std::convert::TryFrom<&DynamicPackage> for Exchange {
    type Error = BuckyError;
    fn try_from(pkg: &DynamicPackage) -> Result<Self, BuckyError> {
        match pkg.cmd_code() {
            PackageCmdCode::SynTunnel => {
                let syn_tunnel: &SynTunnel = pkg.as_ref();
                Ok(Self::from(syn_tunnel))
            }
            PackageCmdCode::TcpSynConnection => {
                let tcp_syn: &TcpSynConnection = pkg.as_ref();
                Ok(Self::from(tcp_syn))
            }
            PackageCmdCode::TcpAckConnection => {
                let tcp_ack: &TcpAckConnection = pkg.as_ref();
                Ok(Self::from(tcp_ack))
            }
            _ => Err(BuckyError::new(
                BuckyErrorCode::InvalidInput,
                "exchange cannt merge with first package",
            )),
        }
    }
}

impl From<&SynTunnel> for Exchange {
    fn from(syn_tunnel: &SynTunnel) -> Self {
        Exchange {
            sequence: syn_tunnel.sequence.clone(),
            seq_key_sign: Signature::default(),
            from_device_id: syn_tunnel.from_device_id.clone(),
            send_time: syn_tunnel.send_time.clone(),
            from_device_desc: syn_tunnel.from_device_desc.clone(),
        }
    }
}

impl From<&TcpSynConnection> for Exchange {
    fn from(tcp_syn: &TcpSynConnection) -> Self {
        Exchange {
            sequence: tcp_syn.sequence.clone(),
            seq_key_sign: Signature::default(),
            from_device_id: tcp_syn.from_device_id.clone(),
            send_time: bucky_time_now(),
            from_device_desc: tcp_syn.from_device_desc.clone(),
        }
    }
}

impl From<&TcpAckConnection> for Exchange {
    fn from(tcp_ack: &TcpAckConnection) -> Self {
        Exchange {
            sequence: tcp_ack.sequence.clone(),
            seq_key_sign: Signature::default(),
            from_device_id: tcp_ack.to_device_desc.desc().device_id(),
            send_time: bucky_time_now(),
            from_device_desc: tcp_ack.to_device_desc.clone(),
        }
    }
}

impl From<&SynProxy> for Exchange {
    fn from(syn_proxy: &SynProxy) -> Self {
        Exchange {
            sequence: syn_proxy.seq,
            seq_key_sign: Signature::default(),
            from_device_id: syn_proxy.from_peer_id.clone(),
            send_time: bucky_time_now(),
            from_device_desc: syn_proxy.from_peer_info.clone(),
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
        let buf = context.encode(buf, &self.seq_key_sign, flags.next())?;
        let buf =
            context.check_encode(buf, "from_device_id", &self.from_device_id, flags.next())?;
        let buf = context.check_encode(buf, "send_time", &self.send_time, flags.next())?;
        let _buf =
            context.check_encode(buf, "device_desc", &self.from_device_desc, flags.next())?;
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
        let (seq_key_sign, buf) = context.decode(buf, flags.next())?;
        let (from_device_id, buf) = context.check_decode(buf, "from_device_id", flags.next())?;
        let (send_time, buf) = context.check_decode(buf, "send_time", flags.next())?;
        let (from_device_desc, buf) = context.check_decode(buf, "device_desc", flags.next())?;

        Ok((
            Self {
                sequence,
                seq_key_sign,
                from_device_id,
                send_time,
                from_device_desc,
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

    let src = Exchange {
        sequence: TempSeq::from(rand::random::<u32>()),
        seq_key_sign: Signature::default(),
        from_device_id: device.desc().device_id(),
        send_time: bucky_time_now(),
        from_device_desc: device,
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
    let (dst, _) =
        Exchange::raw_decode_with_context(dec, &mut merge_context::OtherDecode::default()).unwrap();

    assert_eq!(dst.sequence, src.sequence);
    assert_eq!(dst.from_device_id, src.from_device_id);
    assert_eq!(
        dst.from_device_desc.desc().device_id(),
        src.from_device_desc.desc().device_id()
    )
}

pub struct SynTunnel {
    pub from_device_id: DeviceId,
    pub to_device_id: DeviceId,
    pub sequence: TempSeq,
    pub from_container_id: IncreaseId,
    pub from_device_desc: Device,
    pub send_time: Timestamp,
}

impl Package for SynTunnel {
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
        let buf =
            context.check_encode(buf, "from_device_id", &self.from_device_id, flags.next())?;
        let buf = context.check_encode(buf, "to_device_id", &self.to_device_id, flags.next())?;
        let buf = context.check_encode(buf, "sequence", &self.sequence, flags.next())?;
        let buf = context.encode(buf, &self.from_container_id, flags.next())?;
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
        let (from_device_id, buf) = context.check_decode(buf, "from_device_id", flags.next())?;
        let (to_device_id, buf) = context.check_decode(buf, "to_device_id", flags.next())?;
        let (sequence, buf) = context.check_decode(buf, "sequence", flags.next())?;
        let (from_container_id, buf) = context.decode(buf, flags.next())?;
        let (from_device_desc, buf) = context.check_decode(buf, "device_desc", flags.next())?;
        let (send_time, buf) = context.check_decode(buf, "send_time", flags.next())?;
        Ok((
            Self {
                from_device_id,
                to_device_id,
                sequence,
                from_container_id,
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
        from_device_id: from_device.desc().device_id(),
        to_device_id: to_device.desc().device_id(),
        sequence: TempSeq::from(rand::random::<u32>()),
        from_container_id: IncreaseId::default(),
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
    assert_eq!(dst.from_device_id, src.from_device_id);
    assert_eq!(dst.to_device_id, src.to_device_id);
    assert_eq!(
        dst.from_device_desc.desc().device_id(),
        src.from_device_desc.desc().device_id()
    );
}

pub const ACK_TUNNEL_RESULT_OK: u8 = 0;
pub const ACK_TUNNEL_RESULT_REFUSED: u8 = 1;
pub struct AckTunnel {
    pub sequence: TempSeq,
    pub from_container_id: IncreaseId,
    pub to_container_id: IncreaseId,
    pub result: u8,
    pub send_time: Timestamp,
    pub mtu: u16,
    pub to_device_desc: Device,
}

impl Package for AckTunnel {
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
        let buf = context.check_encode(buf, "sequence", &self.sequence, flags.next())?;
        let buf = context.encode(buf, &self.from_container_id, flags.next())?;
        let buf = context.encode(buf, &self.to_container_id, flags.next())?;
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
        let (sequence, buf) = context.check_decode(buf, "sequence", flags.next())?;
        let (from_container_id, buf) = context.decode(buf, flags.next())?;
        let (to_container_id, buf) = context.decode(buf, flags.next())?;
        let (result, buf) = context.decode(buf, flags.next())?;
        let (send_time, buf) = context.check_decode(buf, "send_time", flags.next())?;
        let (mtu, buf) = context.decode(buf, flags.next())?;
        let (to_device_desc, buf) = context.check_decode(buf, "device_desc", flags.next())?;
        Ok((
            Self {
                sequence,
                from_container_id,
                to_container_id,
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
        sequence: TempSeq::from(rand::random::<u32>()),
        from_container_id: IncreaseId::default(),
        to_container_id: IncreaseId::default(),
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
    assert_eq!(dst.from_container_id, src.from_container_id);
    assert_eq!(dst.to_container_id, src.to_container_id);
    assert_eq!(dst.result, src.result);
    assert_eq!(dst.send_time, src.send_time);
    assert_eq!(dst.mtu, src.mtu);
    assert_eq!(
        dst.to_device_desc.desc().device_id(),
        src.to_device_desc.desc().device_id()
    )
}

pub struct AckAckTunnel {
    seq: TempSeq,
    to_container_id: u32,
}

impl Package for AckAckTunnel {
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
        let buf = context.check_encode(buf, "seq", &self.seq, flags.next())?;
        let _buf =
            context.check_encode(buf, "to_container_id", &self.to_container_id, flags.next())?;
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
        let (to_container_id, buf) = context.check_decode(buf, "to_container_id", flags.next())?;
        Ok((
            Self {
                seq,
                to_container_id,
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
        to_container_id: rand::random::<u32>(),
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
    assert_eq!(dst.to_container_id, src.to_container_id);
}

pub struct PingTunnel {
    pub package_id: u32,
    pub to_container_id: IncreaseId,
    pub send_time: Timestamp,
    pub recv_data: u64,
}

impl Package for PingTunnel {
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
        let buf = context.encode(buf, &self.to_container_id, flags.next())?;
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
        let (package_id, buf) = context.decode(buf, flags.next())?;
        let (to_container_id, buf) = context.decode(buf, flags.next())?;
        let (send_time, buf) = context.decode(buf, flags.next())?;
        let (recv_data, buf) = context.decode(buf, flags.next())?;

        Ok((
            Self {
                package_id,
                to_container_id,
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
        to_container_id: IncreaseId::default(),
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
    assert_eq!(dst.to_container_id, src.to_container_id);
    assert_eq!(dst.send_time, src.send_time);
    assert_eq!(dst.recv_data, src.recv_data);
}

pub struct PingTunnelResp {
    pub ack_package_id: u32,
    pub to_container_id: IncreaseId,
    pub send_time: Timestamp,
    pub recv_data: u64,
}

impl Package for PingTunnelResp {
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
        let buf = context.encode(buf, &self.to_container_id, flags.next())?;
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
        let (ack_package_id, buf) = context.decode(buf, flags.next())?;
        let (to_container_id, buf) = context.decode(buf, flags.next())?;
        let (send_time, buf) = context.decode(buf, flags.next())?;
        let (recv_data, buf) = context.decode(buf, flags.next())?;

        Ok((
            Self {
                ack_package_id,
                to_container_id,
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
        to_container_id: IncreaseId::default(),
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
    assert_eq!(dst.to_container_id, src.to_container_id);
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
            datagram_header_len += mem::size_of::<DeviceId>();
        }
        if self.author.is_some() {
            datagram_header_len += mem::size_of::<Device>();
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
        let (to_vport, buf) = context.decode(buf, flags.next())?;
        let (from_vport, buf) = context.decode(buf, flags.next())?;
        let (dest_zone, buf) = context.option_decode(buf, flags.next())?;
        let (hop_limit, buf) = context.option_decode(buf, flags.next())?;
        let (sequence, buf) = context.option_decode(buf, flags.next())?;
        let (piece, buf) = context.option_decode(buf, flags.next())?;
        let (send_time, buf) = context.option_decode(buf, flags.next())?;
        let (create_time, buf) = context.option_decode(buf, flags.next())?;
        let (author_id, buf) = context.option_decode(buf, flags.next())?;
        let (author, buf) = context.option_decode(buf, flags.next())?;
        let (inner_type, buf) = context.decode(buf, flags.next())?;
        let (data, buf) = context.decode(buf, flags.next())?;

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
}

impl Clone for SessionData {
    fn clone(&self) -> Self {
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
        let (stream_pos, buf) = context.decode(buf, context::FLAG_ALWAYS_DECODE)?;
        let (ack_stream_pos, buf) = context.decode(buf, context::FLAG_ALWAYS_DECODE)?;
        let (sack, buf) = context.option_decode(buf, SESSIONDATA_FLAG_SACK)?;
        let (session_id, buf) = context.decode(buf, context::FLAG_ALWAYS_DECODE)?;
        let (send_time, buf) = context.check_decode(buf, "send_time", SESSIONDATA_FLAG_SENDTIME)?;
        let (syn_info, buf) = context.option_decode(buf, SESSIONDATA_FLAG_SYN)?;
        let (to_session_id, buf) = context.option_decode(buf, SESSIONDATA_FLAG_TO_SESSION_ID)?;
        let (id_part, buf) = context.option_decode(
            buf,
            SESSIONDATA_FLAG_PACKAGEID | SESSIONDATA_FLAG_ACK_PACKAGEID,
        )?;
        let (payload, buf) = context.decode(buf, context::FLAG_ALWAYS_DECODE)?;

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
    pub from_device_id: DeviceId,
    pub to_device_id: DeviceId,
    pub proxy_device_id: Option<DeviceId>,
    pub from_device_desc: Device,
    pub reverse_endpoint: Option<Vec<Endpoint>>,
    pub payload: TailedOwnedData,
}

impl std::fmt::Display for TcpSynConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TcpSynConnection:{{sequence:{:?},to_vport:{},from_device_id:{}, reverse_endpoint:{:?}}}",
            self.sequence, self.to_vport, self.from_device_id, self.reverse_endpoint
        )
    }
}

impl Package for TcpSynConnection {
    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::TcpSynConnection
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
        let buf =
            context.check_encode(buf, "from_device_id", &self.from_device_id, flags.next())?;
        let buf = context.check_encode(buf, "to_device_id", &self.to_device_id, flags.next())?;
        let buf = context.option_encode(buf, &self.proxy_device_id, flags.next())?;
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
        let (result, buf) = context.decode(buf, flags.next())?;
        let (to_vport, buf) = context.decode(buf, flags.next())?;
        let (from_session_id, buf) = context.decode(buf, flags.next())?;
        let (from_device_id, buf) = context.check_decode(buf, "from_device_id", flags.next())?;
        let (to_device_id, buf) = context.check_decode(buf, "to_device_id", flags.next())?;
        let (proxy_device_id, buf) = context.option_decode(buf, flags.next())?;
        let (from_device_desc, buf) = context.check_decode(buf, "device_desc", flags.next())?;
        let (reverse_endpoint, buf) = context.option_decode(buf, flags.next())?;
        let (payload, buf) = context.decode(buf, flags.next())?;

        Ok((
            Self {
                sequence,
                result,
                to_vport,
                from_session_id,
                from_device_id,
                to_device_id,
                proxy_device_id,
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
        from_device_id: from_device.desc().device_id(),
        to_device_id: to_device.desc().device_id(),
        proxy_device_id: Some(from_device.desc().device_id()),
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
    assert_eq!(dst.from_device_id, src.from_device_id);
    assert_eq!(dst.to_device_id, src.to_device_id);
    assert_eq!(dst.proxy_device_id, src.proxy_device_id);

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
    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::TcpAckConnection
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
        let (to_session_id, buf) = context.decode(buf, flags.next())?;
        let (result, buf) = context.decode(buf, flags.next())?;
        let (to_device_desc, buf) = context.check_decode(buf, "device_desc", flags.next())?;
        let (payload, buf) = context.decode(buf, flags.next())?;

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
        let (result, buf) = context.decode(buf, flags.next())?;

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

#[derive(Clone)]
pub struct SnCall {
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

impl Package for SnCall {
    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::SnCall
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
        let (seq, buf) = context.check_decode(buf, "sequence", flags.next())?;
        let (sn_peer_id, buf) = context.check_decode(buf, "sn_device_id", flags.next())?;
        let (to_peer_id, buf) = context.check_decode(buf, "to_device_id", flags.next())?;
        let (from_peer_id, buf) = context.check_decode(buf, "from_device_id", flags.next())?;
        let (reverse_endpoint_array, buf) = context.decode(buf, flags.next())?;
        let (active_pn_list, buf) = context.decode(buf, flags.next())?;
        let (peer_info, buf) = context.check_option_decode(buf, "device_desc", flags.next())?;
        let (send_time, buf) = context.check_decode(buf, "send_time", flags.next())?;
        let (payload, buf) = context.decode(buf, flags.next())?;
        let is_always_call = context.check_flags(flags.next());

        Ok((
            Self {
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

pub struct SnCallResp {
    //sn call的响应包
    pub seq: TempSeq,                 //序列事情
    pub sn_peer_id: DeviceId,         //sn设备id
    pub result: u8,                   //
    pub to_peer_info: Option<Device>, //
}

impl Package for SnCallResp {
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
        let (result, buf) = context.decode(buf, flags.next())?;
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
    pub from_peer_id: DeviceId,
    pub reverse_endpoint_array: Vec<Endpoint>,
    pub active_pn_list: Vec<DeviceId>,
    pub peer_info: Device,
    pub call_seq: TempSeq,
    pub call_send_time: Timestamp,
    pub payload: SizedOwnedData<SizeU16>,
}

impl Package for SnCalled {
    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::SnCalled
    }
}

impl Into<merge_context::OtherDecode> for &SnCalled {
    fn into(self) -> merge_context::OtherDecode {
        let mut context = merge_context::FirstDecode::new();
        merge_context::Decode::set_name(&mut context, "sequence", &self.call_seq);
        merge_context::Decode::set_name(&mut context, "to_device_id", &self.to_peer_id);
        merge_context::Decode::set_name(&mut context, "from_device_id", &self.from_peer_id);
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
        let buf = context.check_encode(buf, "from_device_id", &self.from_peer_id, flags.next())?;
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
        let (from_peer_id, buf) = context.check_decode(buf, "from_device_id", flags.next())?;
        let (reverse_endpoint_array, buf) = context.decode(buf, flags.next())?;
        let (active_pn_list, buf) = context.decode(buf, flags.next())?;
        let (peer_info, buf) = context.check_decode(buf, "device_desc", flags.next())?;
        let (call_seq, buf) = context.check_decode(buf, "sequence", flags.next())?;
        let (call_send_time, buf) = context.check_decode(buf, "send_time", flags.next())?;
        let (payload, buf) = context.decode(buf, flags.next())?;

        Ok((
            Self {
                seq,
                sn_peer_id,
                to_peer_id,
                from_peer_id,
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
        from_peer_id: from_device.desc().device_id(),
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
    assert_eq!(dst.from_peer_id, src.from_peer_id);
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

pub struct SnPing {
    //ln与sn的keepalive包
    pub seq: TempSeq,                          //序列号
    pub sn_peer_id: DeviceId,                  //sn的设备id
    pub from_peer_id: Option<DeviceId>,        //发送者设备id
    pub peer_info: Option<Device>,             //发送者设备信息
    pub send_time: Timestamp,                  //发送时间
    pub contract_id: Option<ObjectId>,         //合约文件对象id
    pub receipt: Option<ReceiptWithSignature>, //客户端提供的服务清单
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
        let (end_point_array, buf) = context.decode(buf, flags.next())?;
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

#[derive(Clone)]
pub struct SynProxy {
    pub seq: TempSeq,
    pub to_peer_id: DeviceId,
    pub to_peer_timestamp: Timestamp,
    pub from_peer_id: DeviceId,
    pub from_peer_info: Device,
    pub key_hash: KeyMixHash,
}

impl std::fmt::Display for SynProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SynProxy:{{sequence:{:?}, to:{:?}, from:{}, key:{}}}",
            self.seq,
            self.to_peer_id,
            self.from_peer_id,
            self.key_hash.to_string()
        )
    }
}

impl Package for SynProxy {
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
        let buf = context.check_encode(buf, "seq", &self.seq, flags.next())?;
        let buf = context.check_encode(buf, "to_device_id", &self.to_peer_id, flags.next())?;
        let buf = context.encode(buf, &self.to_peer_timestamp, flags.next())?;
        let buf = context.check_encode(buf, "from_device_id", &self.from_peer_id, flags.next())?;
        let buf = context.check_encode(buf, "device_desc", &self.from_peer_info, flags.next())?;
        let _buf = context.encode(buf, &self.key_hash, flags.next())?;
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
        let (seq, buf) = context.check_decode(buf, "seq", flags.next())?;
        let (to_peer_id, buf) = context.check_decode(buf, "to_device_id", flags.next())?;
        let (to_peer_timestamp, buf) = context.decode(buf, flags.next())?;
        let (from_peer_id, buf) = context.check_decode(buf, "from_device_id", flags.next())?;
        let (from_peer_info, buf) = context.check_decode(buf, "device_desc", flags.next())?;
        let (key_hash, buf) = context.decode(buf, flags.next())?;

        Ok((
            Self {
                seq,
                to_peer_id,
                to_peer_timestamp,
                from_peer_id,
                from_peer_info,
                key_hash,
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

    let key_mix_hash = AesKey::random().mix_hash(None);
    let src = SynProxy {
        seq: TempSeq::from(rand::random::<u32>()),
        to_peer_id: to_device.desc().device_id(),
        to_peer_timestamp: bucky_time_now(),
        from_peer_id: from_device.desc().device_id(),
        from_peer_info: from_device,
        key_hash: key_mix_hash,
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
    assert_eq!(dst.from_peer_id, src.from_peer_id);

    let dst_peer_info = dst.from_peer_info.to_hex().unwrap();
    let src_peer_info = src.from_peer_info.to_hex().unwrap();
    assert_eq!(dst_peer_info, src_peer_info);

    assert_eq!(dst.key_hash, src.key_hash);
}

#[derive(Debug)]
pub struct AckProxy {
    pub seq: TempSeq,
    pub to_peer_id: DeviceId,
    pub proxy_endpoint: Option<Endpoint>,
    pub err: Option<BuckyErrorCode>,
}

impl Package for AckProxy {
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
        let (seq, buf) = context.decode(buf, flags.next())?;
        let (to_peer_id, buf) = context.decode(buf, flags.next())?;
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

#[derive(PartialEq, Eq)]
pub enum OnPackageResult {
    Continue,
    Handled,
    Break,
}

pub trait OnPackage<T: Package, Context = Option<()>> {
    fn on_package(&self, _pkg: &T, context: Context) -> Result<OnPackageResult, BuckyError>;
}

pub struct DynamicPackage {
    cmd_code: PackageCmdCode,
    package: Box<dyn Any + Send + Sync>,
}

impl<Context: merge_context::Encode> AsRef<dyn RawEncodeWithContext<Context>> for DynamicPackage {
    fn as_ref(&self) -> &(dyn RawEncodeWithContext<Context> + 'static) {
        use super::super::protocol;
        downcast_handle!(self)
    }
}

impl<T: 'static + super::Package + Send + Sync> AsRef<T> for DynamicPackage {
    fn as_ref(&self) -> &T {
        self.as_any().downcast_ref::<T>().unwrap()
    }
}

impl<T: 'static + super::Package + Send + Sync> AsMut<T> for DynamicPackage {
    fn as_mut(&mut self) -> &mut T {
        self.as_mut_any().downcast_mut::<T>().unwrap()
    }
}

impl<'de, Context: merge_context::Decode> RawDecodeWithContext<'de, &mut Context>
    for DynamicPackage
{
    fn raw_decode_with_context(
        buf: &'de [u8],
        merge_context: &mut Context,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let (cmd_code, buf) = u8::raw_decode(buf)?;
        let cmd_code = PackageCmdCode::try_from(cmd_code)?;
        //TOFIX: may use macro
        match cmd_code {
            PackageCmdCode::Exchange => Exchange::raw_decode_with_context(buf, merge_context)
                .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf)),
            PackageCmdCode::SynTunnel => SynTunnel::raw_decode_with_context(buf, merge_context)
                .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf)),
            PackageCmdCode::AckTunnel => AckTunnel::raw_decode_with_context(buf, merge_context)
                .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf)),
            PackageCmdCode::AckAckTunnel => {
                AckAckTunnel::raw_decode_with_context(buf, merge_context)
                    .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf))
            }
            PackageCmdCode::PingTunnel => PingTunnel::raw_decode_with_context(buf, merge_context)
                .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf)),
            PackageCmdCode::PingTunnelResp => {
                PingTunnelResp::raw_decode_with_context(buf, merge_context)
                    .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf))
            }
            PackageCmdCode::SnCall => SnCall::raw_decode_with_context(buf, merge_context)
                .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf)),
            PackageCmdCode::SnCallResp => SnCallResp::raw_decode_with_context(buf, merge_context)
                .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf)),
            PackageCmdCode::SnCalled => SnCalled::raw_decode_with_context(buf, merge_context)
                .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf)),
            PackageCmdCode::SnCalledResp => {
                SnCalledResp::raw_decode_with_context(buf, merge_context)
                    .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf))
            }
            PackageCmdCode::SnPing => SnPing::raw_decode_with_context(buf, merge_context)
                .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf)),
            PackageCmdCode::SnPingResp => SnPingResp::raw_decode_with_context(buf, merge_context)
                .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf)),
            PackageCmdCode::Datagram => Datagram::raw_decode_with_context(buf, merge_context)
                .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf)),
            PackageCmdCode::SessionData => SessionData::raw_decode_with_context(buf, merge_context)
                .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf)),
            PackageCmdCode::TcpSynConnection => {
                TcpSynConnection::raw_decode_with_context(buf, merge_context)
                    .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf))
            }
            PackageCmdCode::TcpAckConnection => {
                TcpAckConnection::raw_decode_with_context(buf, merge_context)
                    .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf))
            }
            PackageCmdCode::TcpAckAckConnection => {
                TcpAckAckConnection::raw_decode_with_context(buf, merge_context)
                    .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf))
            }
            PackageCmdCode::SynProxy => SynProxy::raw_decode_with_context(buf, merge_context)
                .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf)),
            PackageCmdCode::AckProxy => AckProxy::raw_decode_with_context(buf, merge_context)
                .map(|(pkg, buf)| (DynamicPackage::from(pkg), buf)),
            _ => Err(BuckyError::new(
                BuckyErrorCode::InvalidData,
                "package cmd code ",
            )),
        }
    }
}

impl DynamicPackage {
    pub fn cmd_code(&self) -> super::PackageCmdCode {
        self.cmd_code
    }

    pub fn as_any(&self) -> &dyn Any {
        self.package.as_ref()
    }

    pub fn as_mut_any(&mut self) -> &mut dyn Any {
        self.package.as_mut()
    }

    pub fn into_any(self) -> Box<dyn Any + Send + Sync> {
        self.package
    }
}

impl<T: 'static + super::Package + Send + Sync> From<T> for DynamicPackage {
    fn from(p: T) -> Self {
        Self {
            cmd_code: T::cmd_code(),
            package: Box::new(p),
        }
    }
}
