use std::{
    sync::{atomic::AtomicU32, Arc},
    time::Instant,
};

use cyfs_base::{DeviceId, ObjectId, PeopleId, RawEncode};
use cyfs_bdt::{DatagramOptions, DatagramTunnelGuard};
use cyfs_core::GroupRPath;

use crate::{HotstuffMessage, HotstuffPackage};

use super::NONDriverHelper;

#[derive(Clone)]
pub struct Sender {
    datagram: DatagramTunnelGuard,
    vport: u16,
    non_driver: NONDriverHelper,
    local_device_id: ObjectId,
    sequence: Arc<AtomicU32>,
}

impl Sender {
    pub(crate) fn new(
        datagram: DatagramTunnelGuard,
        non_driver: NONDriverHelper,
        local_device_id: ObjectId,
    ) -> Self {
        let vport = datagram.vport();
        Self {
            datagram,
            vport,
            non_driver,
            local_device_id,
            sequence: Arc::new(AtomicU32::new(rand::random::<u32>() & 0x80000000)),
        }
    }

    pub(crate) async fn post_message(
        &self,
        msg: HotstuffMessage,
        rpath: GroupRPath,
        to: &ObjectId,
    ) {
        let remote = match to.obj_type_code() {
            cyfs_base::ObjectTypeCode::Device => DeviceId::try_from(to).unwrap(),
            cyfs_base::ObjectTypeCode::People => {
                let people_id = PeopleId::try_from(to).unwrap();
                match self.non_driver.get_ood(&people_id).await {
                    Ok(device_id) => device_id,
                    Err(e) => {
                        log::warn!("[group-sender] post message to {}, failed err: {:?}", to, e);
                        return;
                    }
                }
            }
            _ => panic!("invalid remote type: {:?}", to.obj_type_code()),
        };

        let pkg = HotstuffPackage::from_msg(msg, rpath);

        let len = pkg.raw_measure(&None).unwrap();
        let mut buf = Vec::with_capacity(len);
        buf.resize(len, 0);
        let remain = pkg.raw_encode(buf.as_mut_slice(), &None).unwrap();

        assert_eq!(
            remain.len(),
            0,
            "[group-sender] {:?}-{} post group message to {:?} encode err, pkg: {:?}, len: {}",
            pkg.rpath(),
            self.local_device_id,
            remote,
            pkg,
            buf.len()
        );

        log::debug!(
            "[group-sender] {:?}-{} post group message to {:?}, pkg: {:?}, len: {}",
            pkg.rpath(),
            self.local_device_id,
            remote,
            pkg,
            buf.len()
        );

        let mut options = DatagramOptions::default();
        options.create_time = Some(Instant::now().elapsed().as_millis() as u64);
        options.sequence = Some(cyfs_bdt::TempSeq::from(
            self.sequence
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        ));

        if let Err(err) = self
            .datagram
            .send_to(buf.as_slice(), &mut options, &remote, self.vport)
        {
            log::warn!(
                "[group-sender] {:?}-{} post group message to {:?} failed, the caller should retry, pkg: {:?}, len: {}",
                pkg.rpath(),
                self.local_device_id,
                remote,
                pkg,
                buf.len()
            );
        }
    }

    pub(crate) async fn broadcast(&self, msg: HotstuffMessage, rpath: GroupRPath, to: &[ObjectId]) {
        futures::future::join_all(
            to.iter()
                .map(|remote| self.post_message(msg.clone(), rpath.clone(), remote)),
        )
        .await;
    }
}
