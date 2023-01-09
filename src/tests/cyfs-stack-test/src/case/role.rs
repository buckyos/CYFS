use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::*;
use zone_simulator::*;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone)]
struct ZoneRoleChangedNotify {
    notified: Arc<AtomicBool>,
}

impl ZoneRoleChangedNotify {
    pub fn new() -> Self {
        Self {
            notified: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_notified(&self) -> bool {
        self.notified.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl
    EventListenerAsyncRoutine<
        RouterEventZoneRoleChangedEventRequest,
        RouterEventZoneRoleChangedEventResult,
    > for ZoneRoleChangedNotify
{
    async fn call(
        &self,
        param: &RouterEventZoneRoleChangedEventRequest,
    ) -> BuckyResult<RouterEventZoneRoleChangedEventResult> {
        warn!("test role recv zone role changed notify! {}", param);

        match self
            .notified
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => {}
            Err(_) => {
                error!("role changed notify more than once!");
                unreachable!();
            }
        }

        let resp = RouterEventResponse {
            call_next: true,
            handled: true,
            response: None,
        };

        Ok(resp)
    }
}

async fn change_active_ood(stack: &SharedCyfsStack) {
    let mut owner = USER1_DATA.get().unwrap().user.people.clone();
    info!("current's owner={}", owner.format_json().to_string());

    {
        let ood_list = owner
            .body_mut()
            .as_mut()
            .unwrap()
            .content_mut()
            .ood_list_mut();
        ood_list.swap(0, 1);
    }
    owner
        .body_mut()
        .as_mut()
        .unwrap()
        .increase_update_time(bucky_time_now());

    // first post without sign, and will fail
    let buf = owner.to_vec().unwrap();
    let req = NONPostObjectOutputRequest::new_router(
        Some(stack.local_device_id().object_id().to_owned()),
        owner.desc().calculate_id(),
        buf,
    );

    let resp = stack.non_service().post_object(req).await;
    assert!(resp.is_err());

    // with sign
    let signer = RsaCPUObjectSigner::new(
        USER1_DATA.get().unwrap().user.sk.public(),
        USER1_DATA.get().unwrap().user.sk.clone(),
    );
    cyfs_base::sign_and_set_named_object_body(
        &signer,
        &mut owner,
        &SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_SELF),
    )
    .await
    .unwrap();

    let buf = owner.to_vec().unwrap();
    let req = NONPostObjectOutputRequest::new_router(
        Some(stack.local_device_id().object_id().to_owned()),
        owner.desc().calculate_id(),
        buf,
    );
    let resp = stack.non_service().post_object(req).await.unwrap();
    assert!(resp.object.is_none());

    info!("change active ood success!");
}

pub async fn test() {
    let user1_ood = TestLoader::get_shared_stack(DeviceIndex::User1OOD);

    let notifier = ZoneRoleChangedNotify::new();
    user1_ood
        .uni_stack()
        .router_events()
        .zone_role_changed_event()
        .add_event("test-role-watcher", 0, Box::new(notifier.clone())).await.unwrap();

    change_active_ood(&user1_ood).await;

    async_std::task::sleep(std::time::Duration::from_secs(20)).await;

    assert!(notifier.is_notified());
    
    info!("test all role case success!")
}
