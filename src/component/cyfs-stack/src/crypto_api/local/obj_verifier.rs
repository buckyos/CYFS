use crate::meta::*;
use cyfs_base::*;
use cyfs_lib::*;

use once_cell::sync::OnceCell;
use std::sync::Arc;

#[derive(Clone)]
pub struct ObjectInfo {
    pub object_id: ObjectId,
    pub object: Arc<AnyNamedObject>,
}

pub struct VerifyObjectInnerRequest {
    // 校验的签名位置
    pub sign_type: VerifySignType,

    // 被校验对象
    pub object: ObjectInfo,

    // 签名来源对象
    pub sign_object: VerifyObjectType,
}

pub struct ObjectVerifier {
    noc: OnceCell<NamedObjectCacheRef>,

    local_device_id: DeviceId,

    meta_cache: Box<dyn MetaCache>,
}

impl ObjectVerifier {
    pub fn new(local_device_id: DeviceId, meta_cache: Box<dyn MetaCache>) -> Self {
        Self {
            noc: OnceCell::new(),
            meta_cache,
            local_device_id,
        }
    }

    pub(crate) fn bind_noc(&self, noc: NamedObjectCacheRef) {
        if let Err(_) = self.noc.set(noc) {
            unreachable!();
        }
    }

    pub fn noc(&self) -> &NamedObjectCacheRef {
        self.noc.get().unwrap()
    }

    pub async fn verify_object(
        &self,
        req: CryptoVerifyObjectInputRequest,
    ) -> BuckyResult<CryptoVerifyObjectInputResponse> {
        let inner_req = VerifyObjectInnerRequest {
            object: ObjectInfo {
                object_id: req.object.object_id,
                object: req.object.clone_object(),
            },
            sign_type: req.sign_type,
            sign_object: req.sign_object,
        };

        let verify_result = self.verify_object_inner(inner_req).await?;
        let resp = CryptoVerifyObjectInputResponse {
            result: verify_result,
        };

        Ok(resp)
    }
    pub async fn verify_object_inner(
        &self,
        req: VerifyObjectInnerRequest,
    ) -> BuckyResult<VerifyObjectResult> {
    
        match req.sign_object {
            VerifyObjectType::Owner => {
                let owner_id = req.object.object.owner().ok_or_else(|| {
                    let msg = format!("verify but object has no owner: {}", req.object.object_id);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::NotFound, msg)
                })?;

                let owner = self.search_object(&owner_id).await.map_err(|e| {
                    error!(
                        "search sign object's owner failed: obj={}, owner={}, {}",
                        req.object.object_id, owner_id, e
                    );
                    e
                })?;

                let sign_object = ObjectInfo {
                    object_id: owner_id,
                    object: Arc::new(owner),
                };

                self.verify_by_object(&req.object, &req.sign_type, sign_object)
                    .await
            }
            VerifyObjectType::Own => {
                self.verify_by_object(&req.object, &req.sign_type, req.object.clone())
                    .await
            }
            VerifyObjectType::Object(mut sign_object) => {
                sign_object.decode_and_verify()?;

                let object_info = match sign_object.object {
                    Some(object) => {
                        ObjectInfo {
                            object_id: sign_object.object_id.to_owned(),
                            object,
                        }
                    }
                    None => {
                        let obj = self.search_object(&sign_object.object_id).await.map_err(|e| {
                            error!(
                                "search sign object failed: obj={}, {}",
                                sign_object.object_id, e
                            );
                            e
                        })?;
                        ObjectInfo {
                            object_id: sign_object.object_id.to_owned(),
                            object: Arc::new(obj),
                        }
                    }
                };

                self.verify_by_object(&req.object, &req.sign_type, object_info)
                    .await
            }
            VerifyObjectType::Sign(_) => {
                let msg = format!("verify signs not support yet! {}", req.object.object_id);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    // 显式的使用object来校验签名是否有效
    pub async fn verify_by_object(
        &self,
        req: &ObjectInfo,
        sign_type: &VerifySignType,
        sign_object: ObjectInfo,
    ) -> BuckyResult<VerifyObjectResult> {
        info!(
            "verify by object: obj={}, sign_obj={}",
            req.object_id, sign_object.object_id
        );

        let pk = sign_object.object.public_key().ok_or_else(|| {
            let msg = format!(
                "object's sign obj has no public key: obj={}, sign object={}",
                req.object_id, sign_object.object_id
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::NotFound, msg)
        })?;

        let mut verify_result = VerifyObjectResult::default();

        let valid = match pk {
            PublicKeyRef::Single(pk) => {
                let verifier = self.new_verifier(pk);
                self.verify_single_key(
                    &sign_object.object_id,
                    &verifier,
                    &req,
                    &sign_type,
                    &mut verify_result,
                )
                .await?
            }
            PublicKeyRef::MN((n, list)) => {
                self.verify_mn_key(
                    &sign_object.object_id,
                    n.to_owned(),
                    &list,
                    &req,
                    &sign_type,
                    &mut verify_result,
                )
                .await?
            }
        };

        verify_result.valid = valid;
        Ok(verify_result)
    }

    fn new_verifier(&self, pk: &PublicKey) -> Box<dyn Verifier> {
        let verifier = RsaCPUObjectVerifier::new(pk.clone());
        Box::new(verifier) as Box<dyn Verifier>
    }

    async fn verify_mn_key(
        &self,
        sign_object_id: &ObjectId,
        n: u8,
        pk_list: &Vec<PublicKey>,
        req: &ObjectInfo,
        verify_type: &VerifySignType,
        verify_result: &mut VerifyObjectResult,
    ) -> BuckyResult<bool> {
        let mut verifiers = Vec::new();
        for pk in pk_list {
            let verifier = self.new_verifier(pk);
            verifiers.push(verifier);
        }

        if verify_type.desc() {
            let mut pass_count: u8 = 0;
            for verifier in &verifiers {
                if let Ok(valid) = self
                    .verify_single_key(
                        sign_object_id,
                        verifier,
                        req,
                        &VerifySignType::Desc,
                        verify_result,
                    )
                    .await
                {
                    if valid {
                        pass_count += 1;
                    }
                }
            }

            if pass_count < n {
                let msg = format!(
                    "verify object desc mn signs but not match! obj={}, m={}, n={}, pass={}",
                    req.object_id,
                    pk_list.len(),
                    n,
                    pass_count,
                );
                error!("{}", msg);

                return Ok(false);
                // return Err(BuckyError::new(BuckyErrorCode::NotMatch, msg));
            }
        }

        if verify_type.body() {
            let mut pass_count = 0;
            for verifier in &verifiers {
                if let Ok(valid) = self
                    .verify_single_key(
                        sign_object_id,
                        verifier,
                        req,
                        &VerifySignType::Body,
                        verify_result,
                    )
                    .await
                {
                    if valid {
                        pass_count += 1;
                    }
                }
            }

            if pass_count < n {
                let msg = format!(
                    "verify object body mn signs but not match! obj={}, m={}, n={}, pass={}",
                    req.object_id,
                    pk_list.len(),
                    n,
                    pass_count,
                );
                error!("{}", msg);

                return Ok(false);
                // return Err(BuckyError::new(BuckyErrorCode::NotMatch, msg));
            }
        }

        return Ok(true);
    }

    async fn verify_single_key(
        &self,
        sign_object_id: &ObjectId,
        verifier: &Box<dyn Verifier>,
        req: &ObjectInfo,
        verify_type: &VerifySignType,
        verify_result: &mut VerifyObjectResult,
    ) -> BuckyResult<bool> {
        let signs = req.object.signs().ok_or_else(|| {
            let msg = format!("object has no signs: obj={}", req.object_id);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::NotFound, msg)
        })?;

        if verify_type.desc() {
            let signs = signs.desc_signs().ok_or_else(|| {
                let msg = format!("object has no desc signs: obj={}", req.object_id);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::NotFound, msg)
            })?;

            let mut ret = false;
            for (index, sign) in signs.iter().enumerate() {
                match AnyNamedObjectVerifyHelper::verify_desc_sign(verifier, &req.object, &sign)
                    .await
                {
                    Ok(v) => {
                        if v {
                            // 保存校验结果
                            let item_result = VerifySignResult {
                                valid: true,
                                index: index as u8,
                                sign_object_id: sign_object_id.to_owned(),
                            };
                            verify_result.desc_signs.push(item_result);

                            ret = true;
                            break;
                        }
                    }
                    Err(e) => {
                        error!(
                            "verify desc sign error! obj={}, sign={:?}, index={}, {}",
                            req.object_id, sign, index, e
                        );
                    }
                }
            }

            if !ret {
                let msg = format!(
                    "verify object desc signs but not match! obj={}, pk={:?}",
                    req.object_id,
                    verifier.public_key()
                );
                warn!("{}", msg);

                return Ok(false);
                // return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }
        }

        if verify_type.body() {
            let signs = signs.body_signs().ok_or_else(|| {
                let msg = format!("object has no body signs: obj={}", req.object_id);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::NotFound, msg)
            })?;

            let mut ret = false;
            for (index, sign) in signs.iter().enumerate() {
                match AnyNamedObjectVerifyHelper::verify_body_sign(verifier, &req.object, &sign)
                    .await
                {
                    Ok(v) => {
                        if v {
                            // 保存校验结果
                            let item_result = VerifySignResult {
                                valid: true,
                                index: index as u8,
                                sign_object_id: sign_object_id.to_owned(),
                            };
                            verify_result.body_signs.push(item_result);

                            ret = true;
                            break;
                        }
                    }
                    Err(e) => {
                        error!(
                            "verify body sign error! obj={}, sign={:?}, {}",
                            req.object_id, sign, e
                        );
                    }
                }
            }

            if !ret {
                let msg = format!(
                    "verify object body signs but not match! obj={}, pk={:?}",
                    req.object_id,
                    verifier.public_key()
                );
                error!("{}", msg);

                return Ok(false);
                // return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }
        }

        info!(
            "verify object signs success! obj={}, pk={:?}, type={:?}",
            req.object_id,
            verifier.public_key(),
            verify_type
        );

        Ok(true)
    }

    async fn search_object(&self, object_id: &ObjectId) -> BuckyResult<AnyNamedObject> {
        debug!("will search object: {}", object_id);
        let object_raw = self.search_object_raw(object_id).await?;

        self.decode_object(object_id, &object_raw)
    }

    fn decode_object(
        &self,
        object_id: &ObjectId,
        object_raw: &Vec<u8>,
    ) -> BuckyResult<AnyNamedObject> {
        let (obj, _) = AnyNamedObject::raw_decode(&object_raw).map_err(|e| {
            error!(
                "decode raw data from meta chain failed! obj={} err={}",
                object_id, e
            );
            e
        })?;

        // 需要校验id是否匹配
        let real_id = obj.object_id();
        if real_id != *object_id {
            let msg = format!("object id not match: except={}, got={}", object_id, real_id);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        Ok(obj)
    }

    // 查找一个owner对象，先从本地查找，再从meta-chain查找
    async fn search_object_raw(&self, object_id: &ObjectId) -> BuckyResult<Vec<u8>> {
        let req = NamedObjectCacheGetObjectRequest {
            object_id: object_id.clone(),
            source: RequestSourceInfo::new_local_system(),
            last_access_rpath: None,
        };
        if let Ok(Some(obj)) = self.noc().get_object(&req).await {
            return Ok(obj.object.object_raw);
        }

        // 从meta查询
        match self.meta_cache.get_object(object_id).await? {
            Some(data) => Ok(data.object_raw),
            None => {
                let msg = format!("object not found from meta: {}", object_id);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
        }
    }
}
