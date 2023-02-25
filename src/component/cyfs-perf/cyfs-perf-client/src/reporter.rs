use std::path::{PathBuf};
use super::store::PerfStore;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_perf_base::*;

use std::sync::Arc;
use std::time::Duration;
use chrono::Local;
use crate::CYFS_PERF_SERVICE_NAME;

// 用以实现统计项的上报
struct PerfReporterInner {
    cyfs_stack: UniCyfsStackRef,
    store: PerfStore,

    // PerfObject的依赖信息
    id: String,
    version: String,
    device_id: DeviceId,
    people_id: ObjectId,
    dec_id: Option<ObjectId>,

    // 上报的目标
    perf_server: Option<DeviceId>,
    save_to_local: bool,
    save_path: Option<PathBuf>,

    // 上报间隔
    report_interval: Duration,
}

impl PerfReporterInner {
    pub fn new(
        id: String,
        version: String,
        device_id: DeviceId,
        people_id: ObjectId,
        dec_id: Option<ObjectId>,
        perf_server: Option<DeviceId>,
        cyfs_stack: UniCyfsStackRef,
        store: PerfStore,
        save_to_local: bool,
        save_to_file: bool,
        report_interval: Duration
    ) -> Self {
        let save_path = if save_to_file {
            let path = if let Some(dec_id) = dec_id {
                cyfs_util::get_app_data_dir(&dec_id.to_string()).join("stat").join(&id)
            } else {
                cyfs_util::get_service_data_dir(CYFS_PERF_SERVICE_NAME).join(&id)
            };
            if !path.exists() {
                if let Err(e) = std::fs::create_dir_all(&path) {
                    error!("create dir {} err {}, disable stat to file", path.display(), e);
                    None
                } else {
                    Some(path)
                }
            } else {
                Some(path)
            }
        } else {
            None
        };
        Self {
            id,
            version,
            device_id,
            people_id,
            dec_id,
            perf_server,
            cyfs_stack,
            store,
            save_to_local,
            save_path,
            report_interval
        }
    }

    pub async fn run(&self) {
        loop {
            async_std::task::sleep(self.report_interval).await;

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

        if let Some(path) = &self.save_path {
            let begin: chrono::DateTime<Local> = bucky_time_to_system_time(data.time_range.begin).into();
            let end: chrono::DateTime<Local> = bucky_time_to_system_time(data.time_range.end).into();
            let pretty_format = "%Y%m%d_%H%M%S";
            let file_name = format!("{}-{}.stat", begin.format(pretty_format), end.format(pretty_format));
            let file_path = path.join(file_name);
            if let Ok(file) = std::fs::File::create(&file_path) {
                if let Err(e) = serde_json::to_writer_pretty(&file, &data) {
                    error!("save perf data to local file failed! file={}, {}", file_path.display(), e);
                }
            }
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
        info!("will report perf: id={}, version={}, target={:?}, perf_object={}",
            self.id, self.version, self.perf_server, perf_id);

        let perf_server = self.perf_server.as_ref().map(|id| id.object_id().to_owned());

        match self.send_perf(perf_obj, perf_server, self.save_to_local).await {
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
    pub async fn send_perf(
        &self,
        obj: Perf,
        target: Option<ObjectId>,
        save_to_local: bool
    ) -> BuckyResult<()>
    {
        let raw= obj.to_vec().unwrap();
        let object_id = obj.desc().calculate_id();

        // 如果存到本地，就直接用put
        if save_to_local {
            let mut req = NONPutObjectOutputRequest::new(
                NONAPILevel::NOC,
                object_id.clone(), raw);
            req.common.dec_id = Some(PERF_DEC_ID.clone());
            match self.cyfs_stack.non_service().put_object(req).await {
                Ok(_) => {
                    info!("### save perf obj {} to local success!", &object_id);
                    Ok(())
                }
                Err(e) => {
                    error!("### save perf obj {} to local failed! {}", &object_id, e);
                    Err(e)
                }
            }?;
            // 存到root_state
            let mut req = RootStateCreateOpEnvOutputRequest::new(ObjectMapOpEnvType::Path);
            req.common.target = target.clone();
            req.common.target_dec_id = self.dec_id.clone();

            let resp = self.cyfs_stack.root_state().create_op_env(req).await?;
            let path_env = PathOpEnvStub::new(resp, target.clone(), self.dec_id.clone());
            let key = obj.get_time_range().to_string();
            path_env.set_with_key("/stat", key, &object_id, None, true).await?;
            path_env.commit().await?;
        } else {
            let mut req = NONPostObjectRequest::new_router(target, object_id.clone(), raw);
            req.common.dec_id = Some(PERF_DEC_ID.clone());
            req.common.req_path = Some(PERF_REPORT_PATH.to_owned());
            let str_target = target.map_or("ood".to_owned(), |id| id.to_string());
            match self.cyfs_stack.non_service().post_object(req).await {
                Ok(_) => {
                    info!("### send perf obj {} to {} success!", &object_id, str_target);
                    Ok(())
                }
                Err(e) => {
                    error!("### send perf obj {} to {} failed! {}", &object_id, str_target, e);
                    Err(e)
                }
            }?;
        }

        Ok(())
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
        cyfs_stack: UniCyfsStackRef,
        store: PerfStore,
        save_to_local: bool,
        save_to_file: bool,
        report_interval: Duration,
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
            save_to_local,
            save_to_file,
            report_interval
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
