use super::store::PerfStore;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_perf_base::*;

use std::sync::Arc;

// 挂载到root-state统计项
struct NocRootStateInner {
    cyfs_stack: SharedCyfsStack,
    store: PerfStore,

    // PerfObject的依赖项
    id: String,
    version: String,
    device_id: DeviceId,
    people_id: ObjectId,
    dec_id: Option<ObjectId>,

    // 上报目标
    perf_server: Option<DeviceId>,
}


impl NocRootStateInner {
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
            async_std::task::sleep(std::time::Duration::from_secs(10)).await;

            let _ = self.root_state().await;
        }
    }

    async fn root_state(&self) -> BuckyResult<()> {
        let data = self.store.clone_data();
        if data.is_empty() {
            return Ok(());
        }

        debug!(
            "noc root state perf data: id={}, version={}, data={:?}",
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

        let perf_obj_id = perf_obj.desc().calculate_id();
        info!(
            "noc root state perf: id={}, version={}, target={:?}, perf_object={}",
            self.id, self.version, self.perf_server, perf_obj_id,
        );

        let perf_server = self
            .perf_server
            .as_ref()
            .map(|id| id.object_id().to_owned());

        // 把对象存到root_state
        let root_state = self.cyfs_stack.root_state_stub(perf_server, self.dec_id);
        let root_info = root_state.get_current_root().await?;
        info!("current root: {:?}", root_info);
        let op_env = root_state.create_path_op_env().await?;
        //format!("{/<perf-dec-id>/local/<DecId>/<isolate_id>/<Date>/<TimeSpanStart>/<id>/<PerfType>}") 
        op_env
            .set_with_key("/perf-dec-id", perf_obj_id.to_string(), &perf_obj_id, None, true)
            .await?;

        let root = op_env.commit().await?;
        info!("new dec root is: {:?}, perf_obj_id={}", root, perf_obj_id);

        Ok(())
    }
}


#[derive(Clone)]
pub(crate) struct NocRootState(Arc<NocRootStateInner>);

impl NocRootState {
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
        let ret = NocRootStateInner::new(
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