mod encode;
mod raptor;
mod range;
pub use encode::*;
pub use raptor::*;
pub use range::*;

pub enum TypedChunkEncoder {
    Range(RangeEncoder), 
    Raptor(RaptorEncoder)
}