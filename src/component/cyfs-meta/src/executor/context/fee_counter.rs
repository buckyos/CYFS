use cyfs_base::*;
use crate::*;

pub struct FeeCounter {
    max_fee: u32,
    fee_used: u32
}

impl FeeCounter {
    pub fn new(max_fee: u32) -> FeeCounter {
        FeeCounter {
            max_fee,
            fee_used: 0
        }
    }
    // 消耗指定的fee
    pub fn cost(&mut self, fee: u32) -> BuckyResult<()> {
        let fee_used = self.fee_used + fee;
        if fee_used > self.max_fee {
            self.fee_used = self.max_fee;
            Err(crate::meta_err!(ERROR_OUT_OF_GAS))
        } else {
            self.fee_used = fee_used;
            Ok(())
        }
    }

    pub fn fee_used(&self) -> u32 {
        self.fee_used
    }

    pub fn max_fee(&self) -> u32 {self.max_fee}
}
