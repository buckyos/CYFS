use super::store::PerfStore;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_perf_base::*;

use std::sync::Arc;

// 用以实现统计项的上报
struct PerfReporterInner {
    cyfs_stack: SharedCyfsStack,
    store: PerfStore,

    // PerfObject的依赖信息
    id: String,
    version: String,
    device_id: DeviceId,
    people_id: ObjectId,
    dec_id: Option<ObjectId>,

    // 上报的目标
    perf_server: Option<DeviceId>,
}

impl PerfReporterInner {
    pub fn new(
        id: String,
        version: String,
        device_id: DeviceId,
        people_id: ObjectId,
        dec_id: Option<ObjectId>,
        perf_server: Option<DeviceId>,
        cyfs_stack: SharedCyfsStack,
        store: PerfStore,
    ) -> Self {
        Self {
            id,
            version,
            device_id,
            people_id,
            dec_id,
            perf_server,
            cyfs_stack,
            store,
        }
    }

    pub async fn run(&self) {
        loop {
            async_std::task::sleep(std::time::Duration::from_secs(60 * 10)).await;

            // 这里先按照等间隔均匀上报，忽略上报的结果
            let _ = self.report_once().await;
        }
    }

    async fn report_once(&self) -> BuckyResult<()> {
        if self.store.lock_for_report() {
            // 已经在上报了，目前不应该发生，因为我们只有一个上报task，不会并发处理
            unreachable!();
        }

        let ret = self.report_impl().await;
        if ret.is_ok() {
            self.store.clear_data();
        }

        self.store.unlock_for_report();

        ret
    }

    async fn report_impl(&self) -> BuckyResult<()> {
        let data = self.store.clone_data();
        if data.is_empty() {
            return Ok(());
        }

        debug!(
            "will report perf data: id={}, version={}, data={:?}",
            self.id, self.version, data
        );

        let perf_obj = Perf::create(
            self.device_id.clone(),
            self.people_id.clone(),
            self.dec_id.clone(),
            self.id.clone(),
            self.version.clone(),
            data,
        );

        let perf_id = perf_obj.perf_id();
        info!(
            "will report perf: id={}, version={}, target={:?}, perf_object={}",
            self.id, self.version, self.perf_server, perf_id,
        );

        let perf_server = self
            .perf_server
            .as_ref()
            .map(|id| id.object_id().to_owned());

        match self.put_object(&perf_obj, perf_server, 0).await {
            Ok(_) => {
                info!(
                    "report perf success! id={}, target={:?}, perf_object={}",
                    self.id, self.perf_server, perf_id
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "report perf failed! id={}, target={:?}, perf_object={}",
                    self.id, self.perf_server, perf_id
                );
                Err(e)
            }
        }
    }

    // NON_REQUEST_FLAG_SIGN_BY_DEVICE | NON_REQUEST_FLAG_SIGN_SET_DESC | NON_REQUEST_FLAG_SIGN_SET_BODY
    pub async fn put_object<D, T, N>(
        &self,
        obj: &N,
        target: Option<ObjectId>,
        sign_flags: u32,
    ) -> BuckyResult<()>
    where
        D: ObjectType,
        T: RawEncode,
        N: RawConvertTo<T>,
        N: NamedObject<D>,
        <D as ObjectType>::ContentType: BodyContent,
    {
        let raw;
        let object_id = obj.desc().calculate_id();
        if sign_flags != 0 {
            let object_raw = obj.to_vec().unwrap();
            let req = CryptoSignObjectRequest::new(object_id.clone(), object_raw, sign_flags);

            // 先给Obj签名, 用Client的Device
            let resp = self
                .cyfs_stack
                .crypto()
                .sign_object(req)
                .await
                .map_err(|e| {
                    error!("{} sign failed, err {}", &object_id, e);
                    e
                })?;
            raw = resp.object.unwrap().object_raw;
        } else {
            raw = obj.to_vec().unwrap();
        }

        // 把obj再put出去
        match self
            .cyfs_stack
            .non_service()
            .put_object(NONPutObjectOutputRequest {
                common: NONOutputRequestCommon {
                    req_path: None,
                    target,
                    dec_id: obj.desc().dec_id().clone(),
                    flags: 0,
                    level: NONAPILevel::Router,
                },
                object: NONObjectInfo {
                    object_id: object_id.clone(),
                    object_raw: raw,
                    object: None,
                },
                access: None,
            })
            .await
        {
            Ok(_) => {
                info!(
                    "### put perf obj [{}] to {} success!",
                    object_id,
                    target.map_or("ood".to_owned(), |id| id.to_string())
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "### put perf obj [{}] to {} failed! {}",
                    object_id,
                    target.map_or("ood".to_owned(), |id| id.to_string()),
                    e
                );
                Err(e)
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct PerfReporter(Arc<PerfReporterInner>);

impl PerfReporter {
    pub fn new(
        id: String,
        version: String,
        device_id: DeviceId,
        people_id: ObjectId,
        dec_id: Option<ObjectId>,
        perf_server: Option<DeviceId>,
        cyfs_stack: SharedCyfsStack,
        store: PerfStore,
    ) -> Self {
        let ret = PerfReporterInner::new(
            id,
            version,
            device_id,
            people_id,
            dec_id,
            perf_server,
            cyfs_stack,
            store,
        );
        Self(Arc::new(ret))
    }

    pub fn start(&self) {
        let this = self.clone();
        async_std::task::spawn(async move {
            this.0.run().await;
        });
    }
}
