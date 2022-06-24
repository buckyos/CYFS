use std::sync::RwLock;

pub struct Status {
    cur_height: RwLock<i64>,
    tx_num: RwLock<u64>,
}

impl Status {
    pub fn new() -> Self {
        Self {
            cur_height: RwLock::new(-1),
            tx_num: RwLock::new(0)
        }
    }

    pub fn cur_height(&self) -> i64 {
        let height = *self.cur_height.read().unwrap();
        height
    }

    pub fn tx_num(&self) -> u64 {
        let num = *self.tx_num.read().unwrap();
        num
    }

    pub fn set_height(&self, height: i64) {
        *self.cur_height.write().unwrap() = height;
    }

    pub fn set_tx_num(&self, tx_num: u64) {
        *self.tx_num.write().unwrap() = tx_num;
    }

    pub fn add_tx_num(&self, tx_num: u64) {
        *self.tx_num.write().unwrap() += tx_num;
    }
}