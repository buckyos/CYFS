use crate::*;

pub struct ObjectDifficulty {}

impl ObjectDifficulty {
    pub fn difficulty(hash: &HashValue) -> u8 {
        let hash = hash.as_slice();
        let mut i = 0;
        let mut diff = 0;
        while i < hash.len() {
            let ret = Self::leading_zero(hash[i]);
            diff += ret;
            if ret < 8 {
                break;
            }

            i += 1;
        }

        diff
    }

    fn leading_zero(byte: u8) -> u8 {
        if byte == 0 {
            8
        } else {
            let mut count = 7;
            loop {
                let tmp = byte & (1 << count);
                // println!("count={},value={:b}", count, tmp);
                if tmp != 0 {
                    break;
                }

                if count == 0 {
                    break;
                }

                count -= 1;
            }

            7 - count as u8
        }
    }

    pub fn format_binary(hash: &[u8]) {
        for n in hash.iter() {
            print!("{:b} ", n);
        }
    }
}


#[cfg(test)]
mod test {
    use crate::*;
    use std::str::FromStr;

    #[test]
    fn calc_diff() {
        assert_eq!(ObjectDifficulty::leading_zero(0), 8);
        assert_eq!(ObjectDifficulty::leading_zero(0b01111111), 1);
        assert_eq!(ObjectDifficulty::leading_zero(0b00111111), 2);
        assert_eq!(ObjectDifficulty::leading_zero(0b00011111), 3);
        assert_eq!(ObjectDifficulty::leading_zero(0b00001111), 4);
        assert_eq!(ObjectDifficulty::leading_zero(0b00000111), 5);
        assert_eq!(ObjectDifficulty::leading_zero(0b00000011), 6);
        assert_eq!(ObjectDifficulty::leading_zero(0b00000001), 7);
        assert_eq!(ObjectDifficulty::leading_zero(0b00000000), 8);

        assert_eq!(ObjectDifficulty::leading_zero(0xFF), 0);
        assert_eq!(ObjectDifficulty::leading_zero(0x0F), 4);

        use rand::Rng;
        let mut nonce: u128 = rand::thread_rng().gen();
        let object_id = ObjectId::from_str("95RvaS58mfCGmpqHWM5xdBmbgmZaAQaq24GcQTQxA7q6").unwrap();
        let mut count: u64 = 0;
        let ins = std::time::Instant::now();
        loop {
            let (diff, hash) = ObjectDifficulty::difficulty(&object_id.as_slice(), &nonce);
            if diff >= 20 {
                ObjectDifficulty::format_binary(hash.as_slice());
                println!("got diff: nonce={}", nonce);
                break;
            }

            nonce += 1;
            count += 1;
            if count % (1000 * 1000) == 0 {
                println!("calcing {}, dur={}s......", count, ins.elapsed().as_secs());
            }
        }
    }
}
