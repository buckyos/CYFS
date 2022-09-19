use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use cyfs_stack_loader::*;
use cyfs_util::*;
use zone_simulator::*;

fn create_device() -> (DeviceId, Device) {
    // 创建一个临时的device

    let area = Area::new(0, 0, 0, 0);
    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let pubic_key = private_key.public();

    let device = Device::new(
        Some(ObjectId::default()),
        UniqueId::default(),
        vec![],
        vec![],
        vec![],
        pubic_key,
        area,
        DeviceCategory::Server,
    )
    .build();

    let device_id = device.desc().device_id();

    (device_id, device)
}

fn new_object(dec_id: &ObjectId, owner: Option<ObjectId>, id: &str) -> Text {
    let mut builder = Text::build(id, "test_crypto", "hello!")
        .no_create_time()
        .dec_id(dec_id.to_owned());
    if let Some(owner) = owner {
        builder = builder.owner(owner);
    }
    builder.build()
}

fn new_dec(name: &str) -> ObjectId {
    let owner_id = &USER1_DATA.get().unwrap().people_id;

    let dec_id = DecApp::generate_id(owner_id.object_id().to_owned(), name);

    info!(
        "generage test_crypto dec_id={}, people={}",
        dec_id, owner_id
    );

    dec_id
}

pub async fn test() {
    let dec_id = new_dec("crypto");
    let stack1 = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    add_handlers_for_stack("user1_ood", &stack1, &dec_id);
    let stack2 = TestLoader::get_shared_stack(DeviceIndex::User2OOD);
    add_handlers_for_stack("user2_ood", &stack2, &dec_id);

    add_acl_handlers_for_stack("user1_ood", "test.crypto.out", &stack1, &dec_id);
    add_acl_handlers_for_stack("user2_ood", "test.crypto.in", &stack2, &dec_id);

    async_std::task::sleep(std::time::Duration::from_secs(2)).await;

    test_sign(&dec_id).await;
    test_sign_by_owner(&dec_id).await;

    info!("test crypto complete!");
}

struct OnPreCryptoSignObject {
    stack: String,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerSignObjectRequest, RouterHandlerSignObjectResult>
    for OnPreCryptoSignObject
{
    async fn call(
        &self,
        param: &RouterHandlerSignObjectRequest,
    ) -> BuckyResult<RouterHandlerSignObjectResult> {
        info!(
            "pre_crypto sign_object: stack={}, request={}",
            self.stack, param.request
        );
        assert!(param.response.is_none());

        let result = RouterHandlerSignObjectResult {
            action: RouterHandlerAction::Pass,
            request: None,
            response: None,
        };

        Ok(result)
    }
}

struct OnPostCryptoSignObject {
    stack: String,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerSignObjectRequest, RouterHandlerSignObjectResult>
    for OnPostCryptoSignObject
{
    async fn call(
        &self,
        param: &RouterHandlerSignObjectRequest,
    ) -> BuckyResult<RouterHandlerSignObjectResult> {
        info!(
            "post_crypto sign_object: stack={}, request={}, response={:?}",
            self.stack, param.request, param.response,
        );
        assert!(param.response.is_some());

        let result = RouterHandlerSignObjectResult {
            action: RouterHandlerAction::Pass,
            request: None,
            response: None,
        };

        Ok(result)
    }
}

struct OnPostCryptoVerifyObject {
    stack: String,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerVerifyObjectRequest, RouterHandlerVerifyObjectResult>
    for OnPostCryptoVerifyObject
{
    async fn call(
        &self,
        param: &RouterHandlerVerifyObjectRequest,
    ) -> BuckyResult<RouterHandlerVerifyObjectResult> {
        info!(
            "post_crypto verify_object: stack={}, request={}, response={:?}",
            self.stack, param.request, param.response,
        );
        assert!(param.response.is_some());

        let result = RouterHandlerVerifyObjectResult {
            action: RouterHandlerAction::Pass,
            request: None,
            response: None,
        };

        Ok(result)
    }
}

struct OnAclRequest {
    stack: String,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerAclRequest, RouterHandlerAclResult> for OnAclRequest {
    async fn call(&self, param: &RouterHandlerAclRequest) -> BuckyResult<RouterHandlerAclResult> {
        info!(
            "acl req: stack={}, request={}, response={:?}",
            self.stack, param.request, param.response,
        );
        assert!(param.response.is_none());

        let result = RouterHandlerAclResult {
            action: RouterHandlerAction::Response,
            request: None,
            response: Some(Ok(AclHandlerResponse {
                access: AclAccess::Accept,
            })),
        };

        Ok(result)
    }
}

fn add_handlers_for_stack(name: &str, stack: &SharedCyfsStack, dec_id: &ObjectId) {
    let filter = format!(
        "dec_id == {} && (protocol == http-local || protocol == http-bdt)",
        dec_id
    );

    // pre_crypto
    let listener = OnPreCryptoSignObject {
        stack: name.to_owned(),
    };

    stack
        .router_handlers()
        .add_handler(
            RouterHandlerChain::PreCrypto,
            "pre-crypto",
            0,
            Some(filter.clone()),
            None,
            RouterHandlerAction::Default,
            Some(Box::new(listener)),
        )
        .unwrap();

    // post_crypto sign_object
    let listener = OnPostCryptoSignObject {
        stack: name.to_owned(),
    };

    stack
        .router_handlers()
        .add_handler(
            RouterHandlerChain::PostCrypto,
            "post-crypto",
            0,
            Some(filter.clone()),
            None,
            RouterHandlerAction::Default,
            Some(Box::new(listener)),
        )
        .unwrap();

    // post_crypto verify_object
    let listener = OnPostCryptoVerifyObject {
        stack: name.to_owned(),
    };

    stack
        .router_handlers()
        .add_handler(
            RouterHandlerChain::PostCrypto,
            "post-verify",
            0,
            Some(filter.clone()),
            None,
            RouterHandlerAction::Default,
            Some(Box::new(listener)),
        )
        .unwrap();

    // acl
    let listener = OnAclRequest {
        stack: name.to_owned(),
    };

    stack
        .router_handlers()
        .add_handler(
            RouterHandlerChain::Acl,
            "acl",
            0,
            Some(filter.clone()),
            None,
            RouterHandlerAction::Default,
            Some(Box::new(listener)),
        )
        .unwrap();
}

fn add_acl_handlers_for_stack(name: &str, acl: &str, stack: &SharedCyfsStack, dec_id: &ObjectId) {
    let filter = format!(
        "dec_id == {} && (protocol == http-local || protocol == http-bdt)",
        dec_id
    );

    // acl
    let listener = OnAclRequest {
        stack: name.to_owned(),
    };

    stack
        .router_handlers()
        .add_handler(
            RouterHandlerChain::Acl,
            acl,
            0,
            Some(filter),
            None,
            RouterHandlerAction::Default,
            Some(Box::new(listener)),
        )
        .unwrap();
}

async fn test_sign(dec_id: &ObjectId) {
    // 创建一个随机对象
    let object = new_object(dec_id, None, "test_crypto");
    let object_raw = object.to_vec().unwrap();
    let id = object.text_id();

    let sign_flags = CRYPTO_REQUEST_FLAG_SIGN_BY_DEVICE
        | CRYPTO_REQUEST_FLAG_SIGN_PUSH_DESC
        | CRYPTO_REQUEST_FLAG_SIGN_PUSH_BODY;
    let mut req = CryptoSignObjectRequest::new(id.object_id().to_owned(), object_raw, sign_flags);
    req.common.dec_id = Some(dec_id.to_owned());
    req.common.req_path = Some("测试签名/tests".to_owned());

    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    open_access(&stack, dec_id).await;

    let resp = stack.crypto().sign_object(req).await.unwrap();
    let object_info = resp.object.unwrap();
    assert_eq!(object_info.object_id, *id.object_id());

    // 校验
    let device = stack.local_device();
    let sign_object = NONSlimObjectInfo {
        object_id: device.desc().object_id(),
        object_raw: Some(device.to_vec().unwrap()),
        object: None,
    };

    let mut verify_req = CryptoVerifyObjectRequest::new_verify_by_object(
        VerifySignType::Both,
        object_info.clone(),
        sign_object,
    );
    verify_req.common.dec_id = Some(dec_id.to_owned());

    let resp = stack.crypto().verify_object(verify_req).await.unwrap();
    assert!(resp.result.valid);

    // 错误校验
    let mut verify_req =
        CryptoVerifyObjectRequest::new_verify_by_owner(VerifySignType::Both, object_info);
    verify_req.common.dec_id = Some(dec_id.to_owned());

    // 由于object没有owner，所以这里会返回错误
    let resp = stack.crypto().verify_object(verify_req).await;
    assert!(resp.is_err());
}

async fn open_access(stack: &SharedCyfsStack, dec_id: &ObjectId) {
    // 开启权限
    let meta = stack.root_state_meta_stub(None, None);
    let mut access = AccessString::new(0);
    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Read);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Read);
    access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Read);
    access.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Call);
    access.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Call);
    access.set_group_permission(AccessGroup::OthersDec, AccessPermission::Call);
    let item = GlobalStatePathAccessItem {
        path: CYFS_CRYPTO_VIRTUAL_PATH.to_owned(),
        access: GlobalStatePathGroupAccess::Specified(GlobalStatePathSpecifiedGroup {
            zone: None,
            zone_category: Some(DeviceZoneCategory::CurrentZone),
            dec: Some(dec_id.clone()),
            access: AccessPermissions::ReadAndWrite as u8,
        }),
    };

    meta.add_access(item).await.unwrap();

}
async fn test_sign_by_owner(dec_id: &ObjectId) {
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let stack2 = TestLoader::get_shared_stack(DeviceIndex::User2OOD);

    let ood1_id = stack.local_device_id();
    // 创建一个随机对象
    let object = new_object(dec_id, Some(ood1_id.object_id().to_owned()), "test_crypto");
    let object_raw = object.to_vec().unwrap();
    let id = object.text_id();

    let sign_flags = CRYPTO_REQUEST_FLAG_SIGN_BY_DEVICE
        | CRYPTO_REQUEST_FLAG_SIGN_PUSH_DESC
        | CRYPTO_REQUEST_FLAG_SIGN_PUSH_BODY;
    let mut req = CryptoSignObjectRequest::new(id.object_id().to_owned(), object_raw, sign_flags);
    req.common.dec_id = Some(dec_id.to_owned());

    let resp = stack.crypto().sign_object(req).await.unwrap();
    let object_info = resp.object.unwrap();
    assert_eq!(object_info.object_id, *id.object_id());

    // 校验, 发往device1进行校验
    let mut verify_req =
        CryptoVerifyObjectRequest::new_verify_by_owner(VerifySignType::Both, object_info.clone());
    let device_stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    verify_req.common.target = Some(device_stack.local_device_id().object_id().to_owned());
    verify_req.common.dec_id = Some(dec_id.to_owned());

    let resp = stack.crypto().verify_object(verify_req).await.unwrap();
    assert!(resp.result.valid);

    // 错误校验
    // 向同zone的device发起校验
    let device = stack2.local_device();
    let sign_object = NONSlimObjectInfo {
        object_id: device.desc().object_id(),
        object_raw: Some(device.to_vec().unwrap()),
        object: None,
    };

    let mut verify_req = CryptoVerifyObjectRequest::new_verify_by_object(
        VerifySignType::Both,
        object_info.clone(),
        sign_object,
    );
    let device_stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    verify_req.common.target = Some(device_stack.local_device_id().object_id().to_owned());
    verify_req.common.dec_id = Some(dec_id.to_owned());

    let resp = stack
        .crypto()
        .verify_object(verify_req.clone())
        .await
        .unwrap();
    assert!(!resp.result.valid);

    // 权限错误, 通过ood1向ood2发起校验请求，触发权限错误
    verify_req.common.target = Some(stack2.local_device_id().object_id().to_owned());
    verify_req.common.req_path = Some("/test_crypto/acl_error".to_owned());
    verify_req.common.dec_id = Some(dec_id.to_owned());

    let ret = stack.crypto().verify_object(verify_req.clone()).await;
    assert!(ret.is_err());
    let err = ret.unwrap_err();
    assert_eq!(err.code(), BuckyErrorCode::PermissionDenied);

    // 动态添加verify权限
    {
        let resp = stack
            .crypto()
            .verify_object(verify_req.clone())
            .await
            .unwrap();
        assert!(!resp.result.valid);
    }

    // 动态添加verify权限，基于handler回调
    {
        let resp = stack
            .crypto()
            .verify_object(verify_req.clone())
            .await
            .unwrap();
        assert!(!resp.result.valid);
    }
}
