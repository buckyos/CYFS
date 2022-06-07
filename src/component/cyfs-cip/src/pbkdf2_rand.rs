use hmac::{Hmac,};
use rand_core::block::{BlockRngCore, BlockRng};
use rand::{CryptoRng, SeedableRng, Error, RngCore};

use sha2::Sha512;
use std::fmt;


const PBKDF2_ROUNDS: u32 = 2048;
const PBKDF2_BYTES: usize = 64;

pub struct Array16<T>([T; PBKDF2_BYTES / std::mem::size_of::<u32>()]);

impl<T> Default for Array16<T>
where
    T: Default,
{
    #[rustfmt::skip]
    fn default() -> Self {
        Self([
            T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(),
            T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(),
        ])
    }
}

impl<T> AsRef<[T]> for Array16<T> {
    fn as_ref(&self) -> &[T] {
        &self.0
    }
}

impl<T> AsMut<[T]> for Array16<T> {
    fn as_mut(&mut self) -> &mut [T] {
        &mut self.0
    }
}

impl<T> Clone for Array16<T>
where
    T: Copy + Default,
{
    fn clone(&self) -> Self {
        let mut new = Self::default();
        new.0.copy_from_slice(&self.0);
        new
    }
}

impl<T> fmt::Debug for Array16<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Array16 {{}}")
    }
}

#[derive(Clone)]
pub struct PBKDF2Core {
    seed: [u8; 32],
    state: Option<[u8; 64]>,
    index: usize,
}

impl fmt::Debug for PBKDF2Core {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PBKDF2Core {{}}")
    }
}

impl BlockRngCore for PBKDF2Core {
    type Item = u32;
    type Results = Array16<u32>;

    #[inline]
    fn generate(&mut self, r: &mut Self::Results) {
        let seed = match &self.state {
            Some(v) => {
                v as &[u8]
            }
            None => &self.seed as &[u8]
        };

        self.index += 1;
        let salt = format!("cyfs-pbkdf2-{}", self.index);
        pbkdf2::pbkdf2::<Hmac<Sha512>>(seed, salt.as_bytes(), 2048, unsafe {
            &mut *(&mut *r as *mut Array16<u32> as *mut [u8; 64])
        });

        /*
        for x in r.as_mut() {
            *x = x.to_le();
        }
        */

        let state: Vec<u8> = r.0.iter().flat_map(|val| val.to_le_bytes()).collect();

        use std::convert::TryInto;
        self.state = Some(state.try_into().unwrap());

        //println!("end gen pbkdf2 {}", self.count);
    }
}

impl SeedableRng for PBKDF2Core {
    type Seed = [u8; 32];
    #[inline]
    fn from_seed(seed: Self::Seed) -> Self {
        PBKDF2Core { seed, state: None, index: 0, }
    }
}

impl CryptoRng for PBKDF2Core {}


#[derive(Clone, Debug)]
pub struct PBKDF2Rng {
    rng: BlockRng<PBKDF2Core>,
}

impl SeedableRng for PBKDF2Rng {
    type Seed = [u8; 32];
    #[inline]
    fn from_seed(seed: Self::Seed) -> Self {
        let core = PBKDF2Core::from_seed(seed);
        Self {
            rng: BlockRng::new(core),
        }
    }
}

impl PBKDF2Rng {
    pub fn count(&self) -> usize {
        self.rng.core.index
    }
}

impl RngCore for PBKDF2Rng {
    fn next_u32(&mut self) -> u32 {
        self.rng.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.rng.next_u64()
    }

    fn fill_bytes(&mut self, bytes: &mut [u8]) {
        self.rng.fill_bytes(bytes)
    }

    fn try_fill_bytes(&mut self, bytes: &mut [u8]) -> Result<(), Error> {
        self.rng.try_fill_bytes(bytes)
    }
}

impl CryptoRng for PBKDF2Rng {}

impl From<PBKDF2Core> for PBKDF2Rng {
    fn from(core: PBKDF2Core) -> Self {
        PBKDF2Rng {
            rng: BlockRng::new(core),
        }
    }
}