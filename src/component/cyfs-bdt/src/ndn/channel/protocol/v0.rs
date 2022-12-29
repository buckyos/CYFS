use std::{
    convert::TryFrom, 
    ops::Range,
};
use serde_json::{Map, Value};
use cyfs_base::*;
use crate::{
    types::*, 
    interface::udp::MTU, 
    protocol::{PackageCmdCode, Package},
    tunnel::{udp::Tunnel as UdpTunnel, DynamicTunnel}, 
    datagram::DatagramOptions
};
use super::super::super::{
    types::*
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd)]
pub enum CommandCode {
    Interest = 0,
    RespInterest = 1,
}


impl TryFrom<u8> for CommandCode {
    type Error = BuckyError;
    fn try_from(v: u8) -> std::result::Result<Self, Self::Error> {
        match v {
            0u8 => Ok(Self::Interest),
            1u8 => Ok(Self::RespInterest),
            _ => Err(BuckyError::new(
                BuckyErrorCode::InvalidParam,
                "invalid channel command value",
            )),
        }
    }
}

pub trait CommandPackage {
    fn command_code() -> CommandCode;
}

struct FlagsEncodeContext {
    flags: u16,
    length: usize,
}

impl FlagsEncodeContext {
    pub fn new<'a>(
        command_code: u8, 
        buf: &'a mut [u8]
    ) -> Result<(Self, &'a mut [u8]), BuckyError> {
        let buf = command_code.raw_encode(buf, &None)?;
        let buf = u16::from(0 as u16).raw_encode(buf, &None)?;
        Ok((
            Self {
                flags: 0,
                length: 3,
            },
            buf,
        ))
    }

    // 不检查是否merge
    pub fn encode<'a, T: RawEncode>(
        &mut self,
        buf: &'a mut [u8],
        value: &T
    ) -> Result<&'a mut [u8], BuckyError> {
        let pre_len = buf.len();
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
            let pre_len = buf.len();
            self.flags |= inc_flags;
            let next_buf = v.raw_encode(buf, &None)?;
            self.length += pre_len - next_buf.len();
            Ok(next_buf)
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


struct FlagsDecodeContext {
    flags: u16, 
}

impl FlagsDecodeContext {
    pub fn new<'a>(
        buf: &'a [u8]
    ) -> Result<(Self, &'a [u8]), BuckyError> {
        let (flags, buf) = u16::raw_decode(buf)?;
        Ok((
            Self {
                flags, 
            },
            buf,
        ))
    }


    // 如果flags 的对应bit是0，会出错
    // TODO: 支持返回Option None
    pub fn decode<'a, T: RawDecode<'a>>(
        &mut self,
        buf: &'a [u8]
    ) -> Result<(T, &'a [u8]), BuckyError> {
        T::raw_decode(buf)
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


struct FlagsCounter {
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


#[derive(Debug, Clone)]
pub struct Interest {
    pub session_id: TempSeq, 
    pub chunk: ChunkId,
    pub prefer_type: ChunkCodecDesc, 
    pub referer: Option<String>,
    pub from: Option<DeviceId>, 
    pub group_path: Option<String>
    // pub link_url: Option<String>,
    // flow_id:Option<u32>,
    // priority: Option<u8>,
    // token : Option<String>,//必要的验证token
    // sign : Option<Vec<u8>>,
}


impl RawEncodeWithContext<DatagramOptions> for Interest {
    fn raw_measure_with_context(
        &self, 
        _options: &mut DatagramOptions, 
        _purpose: &Option<RawEncodePurpose>
    ) -> Result<usize, BuckyError> {
        unimplemented!()
    }
    fn raw_encode_with_context<'a>(
        &self,
        enc_buf: &'a mut [u8],
        options: &mut DatagramOptions,
        _purpose: &Option<RawEncodePurpose>
    ) -> Result<&'a mut [u8], BuckyError> {
        options.sequence = Some(self.session_id);
        let mut flags = FlagsCounter::new();
        let (mut context, buf) = FlagsEncodeContext::new(CommandCode::Interest as u8, enc_buf)?;
        let buf = context.encode(buf, &self.chunk)?;
        let buf = context.encode(buf, &self.prefer_type)?;
        let buf = context.option_encode(buf, &self.referer, flags.next())?;
        let buf = context.option_encode(buf, &self.from, flags.next())?;
        let _ = context.option_encode(buf, &self.group_path, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de> RawDecodeWithContext<'de, &DatagramOptions> for Interest {
    fn raw_decode_with_context(
        buf: &'de [u8],
        options: &DatagramOptions,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let session_id = options.sequence.ok_or_else(|| 
            BuckyError::new(BuckyErrorCode::InvalidData, "Interest package should has sequence"))?;
        let mut flags = FlagsCounter::new();
        let (mut context, buf) = FlagsDecodeContext::new(buf)?;
        let (chunk, buf) = context.decode(buf)?;
        let (prefer_type, buf) = context.decode(buf)?;
        let (referer, buf) = context.option_decode(buf, flags.next())?;
        let (from, buf) = context.option_decode(buf, flags.next())?;
        let (group_path, buf) = context.option_decode(buf, flags.next())?;
        Ok((
            Self {
                session_id, 
                chunk, 
                prefer_type, 
                referer,
                from, 
                group_path
            },
            buf,
        ))
    }
}

impl JsonCodec<Interest> for Interest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_number_field(&mut obj, "session_id", self.session_id.value());
        JsonCodecHelper::encode_string_field(&mut obj, "chunk", &self.chunk);
        JsonCodecHelper::encode_field(&mut obj, "prefer_type", &self.prefer_type);
        JsonCodecHelper::encode_option_string_field(&mut obj, "referer", self.referer.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "from", self.from.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "group_path", self.group_path.as_ref());
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let session_id: u32 = JsonCodecHelper::decode_int_field(obj, "session_id")?;
        Ok(Self {
            session_id: TempSeq::from(session_id), 
            chunk: JsonCodecHelper::decode_string_field(obj, "chunk")?, 
            prefer_type: JsonCodecHelper::decode_field(obj, "prefer_type")?, 
            referer: JsonCodecHelper::decode_option_string_field(obj, "referer")?, 
            from: JsonCodecHelper::decode_option_string_field(obj, "from")?, 
            group_path: JsonCodecHelper::decode_option_string_field(obj, "group_path")?, 
        })
    }
}


#[test]
fn encode_protocol_ineterest() {
    let src = Interest {
        session_id: TempSeq::from(123), 
        chunk: ChunkId::default(),
        prefer_type: ChunkCodecDesc::Stream(None, None, None), 
        referer: Some("referer".to_owned()), 
        from: None, 
        group_path: None
    };

    let mut buf = [0u8; 1500]; 
    let mut options = DatagramOptions::default();
    let _ = src.raw_encode_with_context(&mut buf, &mut options, &None).unwrap();

    let (cmd, dec) = u8::raw_decode(&buf).map(|(code, dec)| (CommandCode::try_from(code).unwrap(), dec)).unwrap();
    assert_eq!(cmd, CommandCode::Interest);
    let (dst, _) = Interest::raw_decode_with_context(dec, &mut options).unwrap();
    assert_eq!(src.chunk, dst.chunk);
    assert_eq!(src.referer, dst.referer);
}



#[derive(Clone, Debug)]
pub struct RespInterest {
    pub session_id: TempSeq, 
    pub chunk: ChunkId,  
    pub err: BuckyErrorCode,
    pub redirect: Option<DeviceId>,
    pub redirect_referer: Option<String>,
    pub to: Option<DeviceId>,
}


impl RawEncodeWithContext<DatagramOptions> for RespInterest {
    fn raw_measure_with_context(
        &self, 
        _options: &mut DatagramOptions, 
        _purpose: &Option<RawEncodePurpose>
    ) -> Result<usize, BuckyError> {
        unimplemented!()
    }
    fn raw_encode_with_context<'a>(
        &self,
        enc_buf: &'a mut [u8],
        options: &mut DatagramOptions,
        _purpose: &Option<RawEncodePurpose>
    ) -> Result<&'a mut [u8], BuckyError> {
        let mut flags = FlagsCounter::new();

        options.sequence = Some(self.session_id);
        // let mut flags = FlagsCounter::new();
        let (mut context, buf) = FlagsEncodeContext::new(CommandCode::RespInterest as u8, enc_buf)?;
        let buf = context.encode(buf, &self.chunk)?;
        let buf = context.encode(buf, &(self.err.into_u16()))?;
        let buf = context.option_encode(buf, &(self.redirect), flags.next())?;
        let buf = context.option_encode(buf, &(self.redirect_referer), flags.next())?;
        let _ = context.option_encode(buf, &(self.to), flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de> RawDecodeWithContext<'de, &DatagramOptions> for RespInterest {
    fn raw_decode_with_context(
        buf: &'de [u8],
        options: &DatagramOptions,
    ) -> Result<(Self, &'de [u8]), BuckyError> {
        let session_id = options.sequence.ok_or_else(|| 
            BuckyError::new(BuckyErrorCode::InvalidData, "RespInterest package should has sequence"))?;
        let mut flags = FlagsCounter::new();

        let (mut context, buf) = FlagsDecodeContext::new(buf)?;
        let (chunk, buf) = context.decode(buf)?;
        let (err, buf) = context.decode::<u16>(buf)?;
        let err = BuckyErrorCode::from(err);
        let (id, buf) = context.option_decode(buf, flags.next())?;
        let (referer, buf) = context.option_decode(buf, flags.next())?;
        let (to, buf) = context.option_decode(buf, flags.next())?;

        Ok((
            Self {
                session_id, 
                chunk, 
                err,
                redirect: id,
                redirect_referer: referer,
                to
            },
            buf,
        ))
    }
}

impl JsonCodec<RespInterest> for RespInterest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_number_field(&mut obj, "session_id", self.session_id.value());
        JsonCodecHelper::encode_string_field(&mut obj, "chunk", &self.chunk);
        let err: u32 = self.err.into();
        JsonCodecHelper::encode_number_field(&mut obj, "err", err);
        JsonCodecHelper::encode_option_string_field(&mut obj, "redirect", self.redirect.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "redirect_referer", self.redirect_referer.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "to", self.to.as_ref());
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let session_id: u32 = JsonCodecHelper::decode_int_field(obj, "session_id")?;
        let err: u32 = JsonCodecHelper::decode_int_field(obj, "err")?;
        Ok(Self {
            session_id: TempSeq::from(session_id), 
            chunk: JsonCodecHelper::decode_string_field(obj, "chunk")?, 
            err: BuckyErrorCode::from(err), 
            redirect: JsonCodecHelper::decode_option_string_field(obj, "redirect")?, 
            redirect_referer: JsonCodecHelper::decode_option_string_field(obj, "redirect_referer")?, 
            to: JsonCodecHelper::decode_option_string_field(obj, "to")?, 
        })
    }
}



pub struct PieceData {
    pub est_seq: Option<TempSeq>,
    pub session_id: TempSeq, 
    pub chunk: ChunkId, 
    pub desc: PieceDesc, 
    pub data: Vec<u8>,
}

impl Package for PieceData {
    fn version(&self) -> u8 {
        0
    }
    
    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::PieceData
    }
}

impl PieceData {
    pub fn max_header_len() -> usize {
        TempSeq::raw_bytes().unwrap()
            + TempSeq::raw_bytes().unwrap()
            + ChunkId::raw_bytes().unwrap()
            + PieceDesc::raw_bytes().unwrap()
    }

    pub fn max_payload() -> usize {
        UdpTunnel::raw_data_max_payload_len() - u8::raw_bytes().unwrap() - Self::max_header_len()
    }

    pub fn encode_header<'a>(
        buf: &'a mut [u8],
        session_id: &TempSeq,  
        chunk: &ChunkId, 
        desc: &PieceDesc) -> BuckyResult<&'a mut [u8]> { 
        let buf = (PieceData::cmd_code() as u8).raw_encode(buf, &None)?;
        let buf = TempSeq::default().raw_encode(buf, &None)?;
        let buf = session_id.raw_encode(buf, &None)?;
        let buf = chunk.raw_encode(buf, &None)?;
        desc.raw_encode(buf, &None)
    }

    pub fn reset_estimate(buf: &mut [u8], est_seq: TempSeq) {
        let _ = est_seq.raw_encode(&mut buf[u8::raw_bytes().unwrap()..], &None).unwrap();
    }

    pub fn decode_from_raw_data(buf: &[u8]) -> BuckyResult<Self> {
        let (est_seq, buf) = TempSeq::raw_decode(buf).map(|(s, buf)| {
            (
                if s == TempSeq::default() {
                    None
                } else {
                    Some(s)
                },
                buf,
            )
        })?;
        let (session_id, buf) = TempSeq::raw_decode(buf)?;
        let (chunk, buf) = ChunkId::raw_decode(buf)?;
        let (desc, data) = PieceDesc::raw_decode(buf)?;
        Ok(Self {
            est_seq,
            session_id, 
            chunk,
            desc,  
            //FIXME: 这里有机会减少一次拷贝
            data: Vec::from(data),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PieceControlCommand {
    Continue,
    Finish, 
    Pause, 
    Cancel,
}


impl RawEncode for PieceControlCommand {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(u8::raw_bytes().unwrap())
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        match self {
            Self::Continue => 0u8.raw_encode(buf, purpose), 
            Self::Finish => 1u8.raw_encode(buf, purpose), 
            Self::Pause => 2u8.raw_encode(buf, purpose), 
            Self::Cancel => 3u8.raw_encode(buf, purpose), 
        }
    }
}


impl<'de> RawDecode<'de> for PieceControlCommand {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (code, buf) = u8::raw_decode(buf)?;
        let command = match code {
            0u8 => Ok(Self::Continue), 
            1u8 => Ok(Self::Finish), 
            2u8 => Ok(Self::Pause),
            3u8 => Ok(Self::Cancel), 
            _ => Err(BuckyError::new(BuckyErrorCode::InvalidData, "invalid piece control command code"))
        }?;
        Ok((command, buf)) 
    }
}


#[derive(Debug)]
pub struct PieceControl {
    pub sequence: TempSeq, 
    pub session_id: TempSeq, 
    pub chunk: ChunkId, 
    pub command: PieceControlCommand, 
    pub max_index: Option<u32>, 
    pub lost_index: Option<Vec<Range<u32>>>
}

impl PieceControl {
    fn max_index_payload() -> usize {
        125
    }

    pub fn split_send(mut self, tunnel: &DynamicTunnel) -> BuckyResult<()> {
        let send_once = |ctrl: PieceControl| {
            let mut buffer = vec![0u8; MTU];
            let len = ctrl.raw_encode(&mut buffer[tunnel.as_ref().raw_data_header_len()..], &None)
                .map(|buf| MTU - buf.len())?;
            tunnel.as_ref().send_raw_data(&mut buffer[..len])?;
            Ok(())
        };

        match self.command {
            PieceControlCommand::Continue => {
                if self.lost_index.is_some() {
                    let lost_index = self.lost_index.as_mut().unwrap();
                    let mut buffer = vec![0u8; MTU];

                    let enc_from = tunnel.as_ref().raw_data_header_len();
                    
                    let mut flags = FlagsCounter::new();
                    let (mut context, buf_ptr) = FlagsEncodeContext::new(PackageCmdCode::PieceControl as u8, &mut buffer[enc_from..])?;
                    let buf_ptr = context.encode(buf_ptr, &self.sequence)?;
                    let buf_ptr = context.encode(buf_ptr, &self.session_id)?;
                    let buf_ptr = context.encode(buf_ptr, &self.chunk)?;
                    let buf_ptr = context.encode(buf_ptr, &self.command)?;
                    let index_from = MTU - buf_ptr.len(); 
                    let buf_ptr = context.option_encode(buf_ptr, &self.max_index, flags.next())?;
                    let _ = context.option_encode(buf_ptr, &Some(vec![0u8; 0]), flags.next())?;
                    let _ = context.finish(&mut buffer[enc_from..])?;
                    
                    for indices in lost_index.chunks(Self::max_index_payload()) {
                        let buf_ptr = if let Some(max_index) = self.max_index {
                            max_index.raw_encode(&mut buffer[index_from..], &None)?
                        } else {
                            &mut buffer[index_from..]
                        };
                        let buf_ptr = indices.raw_encode(buf_ptr, &None)?;

                        let len = MTU - buf_ptr.len();
                        tunnel.as_ref().send_raw_data(&mut buffer[..len])?;
                    } 
                    Ok(())
                } else {
                    send_once(self)
                }
            }, 
            _ => {
                send_once(self)
            }
        }
    }
}

impl Package for PieceControl {
    fn version(&self) -> u8 {
        0
    }

    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::PieceControl
    }
}

impl RawEncode for PieceControl {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        unimplemented!()
    }

    fn raw_encode<'a>(
        &self,
        enc_buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let mut flags = FlagsCounter::new();
        let (mut context, buf) = FlagsEncodeContext::new(PackageCmdCode::PieceControl as u8, enc_buf)?;
        let buf = context.encode(buf, &self.sequence)?;
        let buf = context.encode(buf, &self.session_id)?;
        let buf = context.encode(buf, &self.chunk)?;
        let buf = context.encode(buf, &self.command)?;
        let buf = context.option_encode(buf, &self.max_index, flags.next())?;
        let _buf = context.option_encode(buf, &self.lost_index, flags.next())?;
        context.finish(enc_buf)
    }
}

impl<'de> RawDecode<'de> for PieceControl {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let mut flags = FlagsCounter::new();
        let (mut context, buf) = FlagsDecodeContext::new(buf)?;
        let (sequence, buf) = context.decode(buf)?;
        let (session_id, buf) = context.decode(buf)?;
        let (chunk, buf) = context.decode(buf)?;
        let (command, buf) = context.decode(buf)?;
        let (max_index, buf) = context.option_decode(buf, flags.next())?;
        let (lost_index, buf) = context.option_decode(buf, flags.next())?;
        Ok((Self {
            sequence, 
            session_id, 
            chunk, 
            command, 
            max_index, 
            lost_index
        }, buf))
    }
}


pub struct ChannelEstimate {
    pub sequence: TempSeq, 
    pub recved: u64,
}

impl Default for ChannelEstimate {
    fn default() -> Self {
        Self {
            sequence: TempSeq::default(), 
            recved: 0 
        }
    }
}

impl Package for ChannelEstimate {
    fn version(&self) -> u8 {
        0
    }
    
    fn cmd_code() -> PackageCmdCode {
        PackageCmdCode::ChannelEstimate
    }
}


impl RawFixedBytes for ChannelEstimate {
    fn raw_bytes() -> Option<usize> {
        Some(
            u8::raw_bytes().unwrap()
            + TempSeq::raw_bytes().unwrap()
            + u64::raw_bytes().unwrap())
    }
}

impl RawEncode for ChannelEstimate {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(Self::raw_bytes().unwrap())
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let buf = (Self::cmd_code() as u8).raw_encode(buf, purpose)?;
        let buf = self.sequence.raw_encode(buf, purpose)?;
        self.recved.raw_encode(buf, purpose)
    }
}

impl<'de> RawDecode<'de> for ChannelEstimate {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (sequence, buf) = TempSeq::raw_decode(buf)?;
        let (recved, buf) = u64::raw_decode(buf)?;
        Ok((Self {
            sequence, 
            recved 
        }, buf))
    }
}
