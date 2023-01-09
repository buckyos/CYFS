use async_std::future;
use cyfs_base::*;
use cyfs_debug::Mutex;
use futures::future::{AbortHandle, AbortRegistration, Abortable};
use rand::Rng;
use std::fmt;
use std::{
    hash::{Hash, Hasher},
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH}
};

#[derive(Clone)]
pub struct MixAesKey {
    pub enc_key: AesKey, 
    pub mix_key: AesKey
}

impl std::fmt::Display for MixAesKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "enc {}, mix {}", self.enc_key.to_hex().unwrap(), self.mix_key.to_hex().unwrap())
    }
}


impl MixAesKey {
    pub fn mix_hash(&self) -> KeyMixHash {
        self.mix_key.mix_hash(Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() / 60))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Sequence(u32);

impl Sequence {
    pub fn value(&self) -> u32 {
        self.0
    }
}

impl std::fmt::Debug for Sequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value())
    }
}

impl From<u32> for Sequence {
    fn from(v: u32) -> Self {
        Sequence(v)
    }
}

impl Hash for Sequence {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(self.0)
    }
}

impl RawFixedBytes for Sequence {
    fn raw_bytes() -> Option<usize> {
        u32::raw_bytes()
    }
}

impl RawEncode for Sequence {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        Ok(<u32 as RawFixedBytes>::raw_bytes().unwrap())
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        self.0.raw_encode(buf, purpose)
    }
}

impl<'de> RawDecode<'de> for Sequence {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        u32::raw_decode(buf).map(|(n, buf)| (Self(n), buf))
    }
}

pub struct SequenceGenerator {
    next_seq: Arc<Mutex<u32>>,
}

impl SequenceGenerator {
    pub fn new() -> Self {
        SequenceGenerator {
            next_seq: Arc::new(Mutex::new(1)),
        }
    }

    pub fn generate(&self) -> Sequence {
        let mut next_seq = self.next_seq.lock().unwrap();
        *next_seq += 1;
        Sequence(*next_seq - 1)
    }
}

#[derive(Clone, Copy, Ord, PartialEq, Eq, Debug)]
pub struct TempSeq(u32);

impl TempSeq {
    pub fn value(&self) -> u32 {
        self.0
    }

    fn now(_now: Timestamp) -> u32 {
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as u32;
        let since_2021 = Duration::from_secs((40 * 365 + 9) * 24 * 3600).as_secs() as u32;
        // TODO: 用10年？
        (now - since_2021) * 10
    }

    // fn time_bits() -> usize {
    //     20
    // }
}

impl PartialOrd for TempSeq {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.0 == 0 || other.0 == 0 {
            self.0.partial_cmp(&other.0)
        } else if (std::cmp::max(self.0, other.0) - std::cmp::min(self.0, other.0)) > (u32::MAX / 2)
        {
            Some(if self.0 > other.0 {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            })
        } else {
            self.0.partial_cmp(&other.0)
        }
    }
}

impl Default for TempSeq {
    fn default() -> Self {
        Self(0)
    }
}

impl From<u32> for TempSeq {
    fn from(v: u32) -> Self {
        Self(v)
    }
}

impl Hash for TempSeq {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(self.0)
    }
}

impl RawFixedBytes for TempSeq {
    fn raw_bytes() -> Option<usize> {
        u32::raw_bytes()
    }
}

impl RawEncode for TempSeq {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        Ok(<u32 as RawFixedBytes>::raw_bytes().unwrap())
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        self.0.raw_encode(buf, purpose)
    }
}

impl<'de> RawDecode<'de> for TempSeq {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        u32::raw_decode(buf).map(|(n, buf)| (Self(n), buf))
    }
}

pub struct TempSeqGenerator {
    cur: AtomicU32,
}

impl TempSeqGenerator {
    pub fn new() -> Self {
        let now = TempSeq::now(bucky_time_now());
        Self {
            cur: AtomicU32::new(now),
        }
    }

    pub fn generate(&self) -> TempSeq {
        let v = self.cur.fetch_add(1, Ordering::SeqCst);
        if v == 0 {
            TempSeq(self.cur.fetch_add(1, Ordering::SeqCst))
        } else {
            TempSeq(v)
        }
    }
}

pub type Timestamp = u64;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct IncreaseId(u32);

impl std::fmt::Display for IncreaseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for IncreaseId {
    fn default() -> Self {
        Self::invalid()
    }
}

impl IncreaseId {
    pub fn invalid() -> Self {
        Self(0)
    }

    pub fn is_valid(&self) -> bool {
        *self != Self::invalid()
    }
}

impl RawEncode for IncreaseId {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        Ok(<u32 as RawFixedBytes>::raw_bytes().unwrap())
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        self.0.raw_encode(buf, purpose)
    }
}

impl<'de> RawDecode<'de> for IncreaseId {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        u32::raw_decode(buf).map(|(n, buf)| (Self(n), buf))
    }
}

pub struct IncreaseIdGenerator {
    cur: AtomicU32,
}

impl IncreaseIdGenerator {
    pub fn new() -> Self {
        let mut rng = rand::thread_rng();
        Self {
            cur: AtomicU32::new(rng.gen_range(1, 0x7fffffff)),
        }
    }

    pub fn generate(&self) -> IncreaseId {
        IncreaseId(self.cur.fetch_add(1, Ordering::SeqCst) + 1)
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EndpointPair(Endpoint, Endpoint);

impl std::fmt::Display for EndpointPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{{},{}}}", self.0, self.1)
    }
}

impl From<(Endpoint, Endpoint)> for EndpointPair {
    fn from(ep_pair: (Endpoint, Endpoint)) -> Self {
        assert!(ep_pair.0.is_same_ip_version(&ep_pair.1));
        assert!(ep_pair.0.protocol() == ep_pair.1.protocol());
        Self(ep_pair.0, ep_pair.1)
    }
}

impl EndpointPair {
    pub fn local(&self) -> &Endpoint {
        &self.0
    }

    pub fn remote(&self) -> &Endpoint {
        &self.1
    }

    pub fn protocol(&self) -> Protocol {
        self.0.protocol()
    }

    pub fn is_ipv4(&self) -> bool {
        self.0.addr().is_ipv4()
    }

    pub fn is_ipv6(&self) -> bool {
        self.0.addr().is_ipv6()
    }

    pub fn is_tcp(&self) -> bool {
        self.0.is_tcp() && self.0.addr().port() == 0
    }

    pub fn is_udp(&self) -> bool {
        self.0.is_udp()
    }

    pub fn is_reverse_tcp(&self) -> bool {
        self.0.is_tcp() && self.0.addr().port() != 0
    }
}

pub struct StateWaiter {
    wakers: Vec<AbortHandle>,
}

impl StateWaiter {
    pub fn new() -> Self {
        Self { wakers: vec![] }
    }

    pub fn transfer(&mut self) -> Self {
        let mut waiter = Self::new();
        self.transfer_into(&mut waiter);
        waiter
    }

    pub fn transfer_into(&mut self, waiter: &mut Self) {
        waiter.wakers.append(&mut self.wakers);
    }

    pub fn new_waiter(&mut self) -> AbortRegistration {
        let (waker, waiter) = AbortHandle::new_pair();
        self.wakers.push(waker);
        waiter
    }

    pub async fn wait<T, S: FnOnce() -> T>(waiter: AbortRegistration, state: S) -> T {
        let _ = Abortable::new(future::pending::<()>(), waiter).await;
        state()
    }

    pub fn wake(self) {
        for waker in self.wakers {
            waker.abort();
        }
    }

    pub fn len(&self) -> usize {
        self.wakers.len()
    }
}
