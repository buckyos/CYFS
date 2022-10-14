use cyfs_base::*;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};


const BDT_RANDOM_PORT_BEGIN: u16 = 2000;
const BDT_RANDOM_PORT_END: u16 = 65500;


pub struct RandomPortGenerator;


impl RandomPortGenerator {
    fn hash_value<T>(s: T) -> u64
    where
        T: Hash,
    {
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }

    fn hash_device(device: &str) -> u16 {
        let v = Self::hash_value(device);
        (v % (BDT_RANDOM_PORT_END - BDT_RANDOM_PORT_BEGIN) as u64 + BDT_RANDOM_PORT_END as u64)
            as u16
    }

    pub fn prepare_endpoints(device: &str, endpoint: &mut Vec<Endpoint>) -> BuckyResult<()> {
        let need_random = endpoint.iter().find(|ep| ep.addr().port() == 0).is_some();
        if !need_random {
            return Ok(());
        }

        let port = Self::select_default_port(device)?;
        endpoint.iter_mut().for_each(|ep| {
            if ep.addr().port() == 0 {
                ep.mut_addr().set_port(port);
            }
        });

        Ok(())
    }

    fn select_default_port(device: &str) -> BuckyResult<u16> {
        let mut port = Self::hash_device(device);

        info!("begin select bdt random port at {}", port);

        let mut total = 0;
        loop {
            if Self::is_port_valid(port) {
                info!("select bdt random port success: {}", port);
                break Ok(port);
            }

            total += 1;
            if total > (BDT_RANDOM_PORT_END - BDT_RANDOM_PORT_BEGIN) {
                let msg = format!("select bdt random port out of range!");
                error!("{}", msg);
                break Err(BuckyError::new(BuckyErrorCode::AddrInUse, msg));
            }

            port += 1;
            if port > BDT_RANDOM_PORT_END {
                port = BDT_RANDOM_PORT_BEGIN;
            }
        }
    }

    fn is_port_valid(port: u16) -> bool {
        use std::net::TcpListener;
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(_) => true,
            _ => false,
        }
    }
}
