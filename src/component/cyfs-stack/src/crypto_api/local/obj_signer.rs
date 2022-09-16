use crate::resolver::DeviceCache;
use crate::zone::*;
use cyfs_base::*;
use cyfs_bdt::StackGuard;
use cyfs_lib::*;

use std::sync::Arc;

pub struct ObjectSignRequest {
    pub object_id: ObjectId,

    // 所属dec
    pub dec_id: Option<ObjectId>,

    // 来源device
    pub source: DeviceId,

    pub object_raw: Vec<u8>,
    pub object: Arc<AnyNamedObject>,

    pub flags: u32,
}

pub struct ObjectSigner {
    zone_manager: ZoneManagerRef,
    device_manager: Box<dyn DeviceCache>,

    signer: Box<dyn Signer>,
    verifier: Box<dyn Verifier>,
    sign_object_id: ObjectId,
}

impl ObjectSigner {
    pub(crate) fn new(
        zone_manager: ZoneManagerRef,
        device_manager: Box<dyn DeviceCache>,
        bdt_stack: &StackGuard,
    ) -> Self {
        let (sign_object_id, signer, verifier) = Self::new_local_device_signer(&bdt_stack);

        Self {
            zone_manager,
            device_manager,

            sign_object_id,
            signer,
            verifier,
        }
    }

    fn new_local_device_signer(
        bdt_stack: &StackGuard,
    ) -> (ObjectId, Box<dyn Signer>, Box<dyn Verifier>) {
        let sk = bdt_stack.keystore().private_key();
        let pk = bdt_stack.keystore().public_key();

        let signer = RsaCPUObjectSigner::new(pk.clone(), sk.clone());
        let verifier = RsaCPUObjectVerifier::new(pk.clone());

        (
            bdt_stack.local_device_id().object_id().to_owned(),
            Box::new(signer),
            Box::new(verifier),
        )
    }

    pub fn need_sign(&self, flags: &u32) -> bool {
        (flags & CRYPTO_REQUEST_FLAG_SIGN_BY_PEOPLE != 0)
            || (flags & CRYPTO_REQUEST_FLAG_SIGN_BY_DEVICE != 0)
    }

    pub async fn sign_object(
        &self,
        req: CryptoSignObjectInputRequest,
    ) -> BuckyResult<CryptoSignObjectInputResponse> {
        // 至少要指定一个要使用的签名源
        if !self.need_sign(&req.flags) {
            let msg = format!(
                "invalid sign flags, sign object support by device/people! flags={}",
                req.flags
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        // TODO 校验来源device，只有同zone的才可以签名
        if req.flags & CRYPTO_REQUEST_FLAG_SIGN_BY_PEOPLE != 0 {
            info!(
                "will pending sign by people: obj={}, flags={}, source={}",
                req.object.object_id, req.flags, req.common.source
            );

            // TODO 这里需要等待people签名
        }

        if req.flags & CRYPTO_REQUEST_FLAG_SIGN_BY_DEVICE != 0 {
            self.process_sign_by_device(req).await
        } else {
            Ok(CryptoSignObjectInputResponse {
                result: SignObjectResult::Pending,
                object: None,
            })
        }
    }

    async fn process_sign_by_device(
        &self,
        req: CryptoSignObjectInputRequest,
    ) -> BuckyResult<CryptoSignObjectInputResponse> {
        info!(
            "will sign object by current device now: obj={}, flags={}, source={}",
            req.object.object_id, req.flags, req.common.source
        );

        let sign_source = self
            .calc_sign_source(&req.object, &self.sign_object_id)
            .await
            .map_err(|e| {
                let msg = format!(
                    "calc sign source error! req obj={}, sign obj={}, err={}",
                    req.object.object_id, self.sign_object_id, e
                );
                error!("{}", msg);

                BuckyError::new(e.code(), msg)
            })?;
        info!(
            "calc sign source: source_obj={}, source={:?}",
            self.sign_object_id, sign_source
        );

        // 对目标对象进行签名
        let mut result_object = req.object.object().as_ref().clone();

        let mut sign_any = false;

        // 尝试对desc签名
        let sign_ret = if req.flags & CRYPTO_REQUEST_FLAG_SIGN_SET_DESC != 0 {
            debug!("will sign and set desc: obj={}", req.object.object_id);

            Some(
                AnyNamedObjectSignHelper::sign_and_set_desc(
                    &self.signer,
                    &mut result_object,
                    &sign_source,
                )
                .await,
            )
        } else if req.flags & CRYPTO_REQUEST_FLAG_SIGN_PUSH_DESC != 0 {
            debug!("will sign and push desc: obj={}", req.object.object_id);

            Some(
                AnyNamedObjectSignHelper::sign_and_push_desc(
                    &self.signer,
                    &mut result_object,
                    &sign_source,
                )
                .await,
            )
        } else {
            None
        };

        match sign_ret {
            Some(ret) => {
                ret.map_err(|e| {
                    let msg = format!(
                        "sign desc error! req obj={}, sign obj={}, err={}",
                        req.object.object_id, self.sign_object_id, e
                    );
                    error!("{}", msg);
                    BuckyError::new(e.code(), msg)
                })?;
                sign_any = true;
            }
            None => {}
        }

        // 尝试对body签名
        let sign_ret = if req.flags & CRYPTO_REQUEST_FLAG_SIGN_SET_BODY != 0 {
            debug!("will sign and set body: obj={}", req.object.object_id);

            Some(
                AnyNamedObjectSignHelper::sign_and_set_body(
                    &self.signer,
                    &mut result_object,
                    &sign_source,
                )
                .await,
            )
        } else if req.flags & CRYPTO_REQUEST_FLAG_SIGN_PUSH_BODY != 0 {
            debug!("will sign and push body: obj={}", req.object.object_id);

            Some(
                AnyNamedObjectSignHelper::sign_and_push_body(
                    &self.signer,
                    &mut result_object,
                    &sign_source,
                )
                .await,
            )
        } else {
            None
        };

        match sign_ret {
            Some(ret) => {
                ret.map_err(|e| {
                    let msg = format!(
                        "sign body error! req obj={}, sign obj={}, err={}",
                        req.object.object_id, self.sign_object_id, e
                    );
                    error!("{}", msg);
                    BuckyError::new(e.code(), msg)
                })?;

                sign_any = true;
            }
            None => {}
        }

        let mut resp = CryptoSignObjectInputResponse {
            result: SignObjectResult::Signed,
            object: None,
        };

        if sign_any {
            debug!(
                "object sign updated! now will encode object: obj={}",
                req.object.object_id
            );

            let object_raw = result_object.to_vec()?;
            let object = NONObjectInfo::new(
                req.object.object_id,
                object_raw,
                Some(Arc::new(result_object)),
            );

            resp.object = Some(object);
        }

        Ok(resp)
    }

    // 计算object签名在所属zone里面ood列表的索引，依赖people->ood_list字段的顺序
    // 如果一个被签名对象没有owner，那么就不会进行这一步的压缩操作
    async fn calc_sign_zone_ood_index_source(
        &self,
        req: &NONObjectInfo,
        sign_object_id: &ObjectId,
    ) -> BuckyResult<Option<SignatureSource>> {
        // 只对device尝试计算index
        if sign_object_id.obj_type_code() != ObjectTypeCode::Device {
            return Ok(None);
        }

        let device_id: DeviceId = sign_object_id.try_into().unwrap();

        // object所在zone就是owner所在的zone
        let owner = req.object().owner();
        if owner.is_none() {
            return Ok(None);
        }

        let owner = owner.unwrap();

        // 如果只是获取zone失败，那么尝试使用非压缩的sign source
        let ret = self.zone_manager.get_zone_by_owner(&owner, None).await;
        if let Err(e) = ret {
            // TODO 如果获取zone失败，仍继续？
            warn!(
                "get zone by owner error! obj={}, owner={}, {}",
                req.object_id, owner, e,
            );
            return Ok(None);
        }
        let zone = ret.unwrap();

        let ret = self
            .zone_manager
            .get_device_zone_ood_index(&zone, &device_id)
            .await;
        if let Err(e) = ret {
            warn!(
                "get zone device index error! obj={}, device={}, {}",
                req.object_id, device_id, e,
            );
            return Ok(None);
        }

        let ood_index = ret.unwrap();
        if ood_index >= std::usize::MAX {
            return Ok(None);
        }

        if ood_index >= 255 {
            error!(
                "ood index in zone entend limit: device={}, index={}",
                device_id, ood_index
            );

            return Ok(None);
        }

        let ood_index = ood_index as u8;
        let index = SIGNATURE_SOURCE_REFINDEX_ZONE_OOD_BEGIN - ood_index;
        if index < SIGNATURE_SOURCE_REFINDEX_ZONE_OOD_END {
            error!(
                "ood index in zone entend sign ref index limit: device={}, index={}",
                device_id, index
            );
            return Ok(None);
        }

        info!("ood index in zone: device={}, index={}", device_id, index);
        Ok(Some(SignatureSource::RefIndex(index)))
    }

    async fn calc_sign_source(
        &self,
        req: &NONObjectInfo,
        sign_object_id: &ObjectId,
    ) -> BuckyResult<SignatureSource> {
        let object = req.object();
        if let Some(owner) = object.owner() {
            if owner == sign_object_id {
                return Ok(SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_OWNER));
            }
        }

        // 判断是不是自签名
        if req.object_id == *sign_object_id {
            return Ok(SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_SELF));
        }

        if let Some(author) = object.author() {
            if author == sign_object_id {
                return Ok(SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_AUTHOR));
            }
        }

        // 如果签名的设备类型是device，判断是不是所属zone的ood list里面一个
        if let Some(src) = self
            .calc_sign_zone_ood_index_source(req, sign_object_id)
            .await?
        {
            return Ok(src);
        }

        // 判断是不是在obj_refs里面
        if let Some(list) = object.ref_objs() {
            for i in 0..list.len() {
                if list[i].obj_id == *sign_object_id {
                    info!(
                        "sign object in ref_obj: sign object={}, index={}",
                        sign_object_id, i
                    );

                    let index = SIGNATURE_SOURCE_REFINDEX_REF_OBJ_BEGIN as usize + i;
                    if index > SIGNATURE_SOURCE_REFINDEX_REF_OBJ_END as usize {
                        let msg = format!("sign object index in ref_objs entend sign ref index limit: sign_obj={}, index={}", sign_object_id, index);
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }

                    return Ok(SignatureSource::RefIndex(index as u8));
                }
            }
        }

        // 直接使用object_link
        let link = ObjectLink {
            obj_id: sign_object_id.to_owned(),
            obj_owner: None,
        };

        info!(
            "will use as object link sign source: req obj={}, sign obj={}",
            req.object_id, sign_object_id
        );

        Ok(SignatureSource::Object(link))
    }

    /*
    async fn prepare_sign_param(
        &self,
        req: &ObjectSignRequest,
    ) -> BuckyResult<RouterEventDefaultParam> {
        let device = self.device_manager.search(&req.source).await?;
        let direction = self
            .zone_manager
            .get_zone_direction(&req.source, Some(device.clone()), true)
            .await?;

        let base = RouterEventDefaultParam {
            object_id: req.object_id.clone(),

            device_id: req.source.clone(),
            device,

            object: Some(req.object.clone()),
            object_raw: Some(req.object_raw.clone()),

            direction,
        };

        Ok(base)
    }

    async fn on_pre_sign_event(
        &self,
        req: &ObjectSignRequest,
        default_action: RouterAction,
    ) -> BuckyResult<RouterAction> {
        if self.rules_manager.rules().pre_request_signs.is_empty() {
            return Ok(default_action);
        }

        let base = self.prepare_sign_param(req).await?;
        let param = RouterEventPreRequestSignParam { base };

        let action = self
            .rules_manager
            .rules()
            .pre_request_signs
            .emit(param, default_action)
            .await;

        Ok(action)
    }

    async fn on_pre_sign(
        &self,
        req: &ObjectSignRequest,
        default_action: RouterAction,
    ) -> BuckyResult<()> {
        let action = self.on_pre_sign_event(req, default_action).await?;

        match action {
            RouterAction::Reject => {
                let msg = format!(
                    "pre_request_signs reject: obj={}, flags={}, source={}",
                    req.object_id, req.flags, req.source,
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::Reject, msg))
            }

            RouterAction::Drop => {
                let msg = format!(
                    "pre_request_signs ignored: obj={}, flags={}, source={}",
                    req.object_id, req.flags, req.source
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::Ignored, msg))
            }

            _ => {
                info!(
                    "pre_request_signs: obj={}, flags={}, source={}, action={}",
                    req.object_id, req.flags, req.source, action,
                );
                Ok(())
            }
        }
    }

    async fn on_post_sign_event(
        &self,
        req: &ObjectSignRequest,
        default_action: RouterAction,
    ) -> BuckyResult<RouterAction> {
        if self.rules_manager.rules().post_request_signs.is_empty() {
            return Ok(default_action);
        }

        let base = self.prepare_sign_param(req).await?;
        let param = RouterEventPostRequestSignParam { base };

        let action = self
            .rules_manager
            .rules()
            .post_request_signs
            .emit(param, default_action)
            .await;

        Ok(action)
    }

    async fn on_post_sign(
        &self,
        req: &ObjectSignRequest,
        default_action: RouterAction,
    ) -> BuckyResult<()> {
        let action = self.on_post_sign_event(req, default_action).await?;

        match action {
            RouterAction::Reject => {
                let msg = format!(
                    "post_request_signs reject: obj={}, flags={}, source={}",
                    req.object_id, req.flags, req.source,
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::Reject, msg))
            }

            RouterAction::Drop => {
                let msg = format!(
                    "post_request_signs ignored: obj={}, flags={}, source={}",
                    req.object_id, req.flags, req.source
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::Ignored, msg))
            }

            _ => {
                info!(
                    "post_request_signs: obj={}, flags={}, source={}, action={}",
                    req.object_id, req.flags, req.source, action,
                );
                Ok(())
            }
        }
    }
    */
}
