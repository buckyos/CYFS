use crate::*;

use async_trait::async_trait;

// 提供一个默认签名器实现

pub struct RsaCPUObjectSigner {
    public_key: PublicKey,
    secret: PrivateKey,
}

impl RsaCPUObjectSigner {
    pub fn new(public_key: PublicKey, secret: PrivateKey) -> Self {
        RsaCPUObjectSigner { public_key, secret }
    }
}

#[async_trait]
impl Signer for RsaCPUObjectSigner {
    fn public_key(&self) -> &PublicKey {
        return &self.public_key;
    }

    async fn sign(&self, data: &[u8], sign_source: &SignatureSource) -> BuckyResult<Signature> {
        let sig = self.secret.sign(data, sign_source.clone());
        Ok(sig)
    }
}

// 提供对 NamedObject 的 desc 和 body 的签名辅助函数
//TODO：这是一个helper函数，要注意使用场景。这里的含义是帮助一个 单所有权或单主 obj完成对desc,body的签名（如果本地能得到signer)
//      现在没对obj是否符合上述有权类型约束进行判断
pub async fn sign_and_set_named_object<D, S, N>(
    signer: &S,
    obj: &mut N,
    sign_source: &SignatureSource,
) -> BuckyResult<()>
where
    D: ObjectType,
    D::DescType: RawEncode,
    D::ContentType: RawEncode + BodyContent,
    S: Signer,
    N: NamedObject<D>,
{
    let desc_sign = sign_named_object_desc(signer, obj, sign_source).await?;
    obj.signs_mut().set_desc_sign(desc_sign);
    if obj.body().is_some() {
        let body_sign = sign_named_object_body(signer, obj, sign_source).await?;
        obj.signs_mut().set_body_sign(body_sign);
    }
    Ok(())
}

//TODO：这是一个helper函数，要注意使用场景。这里的含义是帮助一个 单所有权或单主 obj完成对desc的签名（如果本地能得到signer)
//      现在没对obj是否符合上述有权类型约束进行判断
pub async fn sign_and_set_named_object_desc<D, S, N>(
    signer: &S,
    obj: &mut N,
    sign_source: &SignatureSource,
) -> BuckyResult<()>
where
    D: ObjectType,
    D::DescType: RawEncode,
    D::ContentType: RawEncode + BodyContent,
    S: Signer,
    N: NamedObject<D>,
{
    let sign = sign_named_object_desc(signer, obj, sign_source).await?;
    obj.signs_mut().set_desc_sign(sign);
    Ok(())
}

//TODO：这是一个helper函数，要注意使用场景。这里的含义是帮助一个 单所有权或单主 obj完成对body的签名（如果本地能得到signer)
//      现在没对obj是否符合上述有权类型约束进行判断
pub async fn sign_and_set_named_object_body<D, S, N>(
    signer: &S,
    obj: &mut N,
    sign_source: &SignatureSource,
) -> BuckyResult<()>
where
    D: ObjectType,
    D::DescType: RawEncode,
    D::ContentType: RawEncode + BodyContent,
    S: Signer,
    N: NamedObject<D>,
{
    let body_sign = sign_named_object_body(signer, obj, sign_source).await?;
    obj.signs_mut().set_body_sign(body_sign);
    Ok(())
}

// 这里不会检查签名是否重复，需要调用者自己注意
// 这里一定会分别push一个desc的签名和body的签名！需要单独增加签名的需求要使用下边两个helper函数
pub async fn sign_and_push_named_object<D, S, N>(
    signer: &S,
    obj: &mut N,
    sign_source: &SignatureSource,
) -> BuckyResult<()>
where
    D: ObjectType,
    D::DescType: RawEncode,
    D::ContentType: RawEncode + BodyContent,
    S: Signer,
    N: NamedObject<D>,
{
    let desc_sign = sign_named_object_desc(signer, obj, sign_source).await?;
    obj.signs_mut().push_desc_sign(desc_sign);
    if obj.body().is_some() {
        let body_sign = sign_named_object_body(signer, obj, sign_source).await?;
        obj.signs_mut().push_body_sign(body_sign);
    }
    Ok(())
}

// 这里不会检查签名是否重复，需要调用者自己注意
pub async fn sign_and_push_named_object_desc<D, S, N>(
    signer: &S,
    obj: &mut N,
    sign_source: &SignatureSource,
) -> BuckyResult<()>
where
    D: ObjectType,
    D::DescType: RawEncode,
    D::ContentType: RawEncode + BodyContent,
    S: Signer,
    N: NamedObject<D>,
{
    let body_sign = sign_named_object_desc(signer, obj, sign_source).await?;
    obj.signs_mut().push_desc_sign(body_sign);
    Ok(())
}

// 这里不会检查签名是否重复，需要调用者自己注意
pub async fn sign_and_push_named_object_body<D, S, N>(
    signer: &S,
    obj: &mut N,
    sign_source: &SignatureSource,
) -> BuckyResult<()>
where
    D: ObjectType,
    D::DescType: RawEncode,
    D::ContentType: RawEncode + BodyContent,
    S: Signer,
    N: NamedObject<D>,
{
    let body_sign = sign_named_object_body(signer, obj, sign_source).await?;
    obj.signs_mut().push_body_sign(body_sign);
    Ok(())
}

pub async fn sign_named_object_desc<D, S, N>(
    signer: &S,
    obj: &N,
    sign_source: &SignatureSource,
) -> BuckyResult<Signature>
where
    D: ObjectType,
    D::DescType: RawEncode,
    D::ContentType: RawEncode + BodyContent,
    S: Signer,
    N: NamedObject<D>,
{
    let hash_value = obj.desc().raw_hash_value()?;
    signer.sign(hash_value.as_slice(), sign_source).await
}

pub async fn sign_named_object_body<D, S, N>(
    signer: &S,
    obj: &N,
    sign_source: &SignatureSource,
) -> BuckyResult<Signature>
where
    D: ObjectType,
    D::DescType: RawEncode,
    D::ContentType: RawEncode + BodyContent,
    S: Signer,
    N: NamedObject<D>,
{
    let hash_value = obj.body().as_ref().unwrap().raw_hash_value()?;
    signer.sign(hash_value.as_slice(), sign_source).await
}

pub struct AnyNamedObjectSignHelper;

impl AnyNamedObjectSignHelper {
    pub async fn sign_and_set<S>(
        signer: &S,
        obj: &mut AnyNamedObject,
        sign_source: &SignatureSource,
    ) -> BuckyResult<()>
    where
        S: Signer,
    {
        Self::sign_and_set_desc(signer, obj, sign_source).await?;

        if obj.has_body()? {
            Self::sign_and_set_body(signer, obj, sign_source).await?;
        }

        Ok(())
    }
    //TODO：这是一个helper函数，要注意使用场景。这里的含义是帮助一个 单所有权或单主 obj完成对desc的签名（如果本地能得到signer)
    //      现在没对obj是否符合上述有权类型约束进行判断
    pub async fn sign_and_set_desc<S>(
        signer: &S,
        obj: &mut AnyNamedObject,
        sign_source: &SignatureSource,
    ) -> BuckyResult<()>
    where
        S: Signer,
    {
        let sign = Self::sign_desc(signer, obj, sign_source).await?;
        obj.signs_mut().unwrap().set_desc_sign(sign);

        Ok(())
    }

    //TODO：这是一个helper函数，要注意使用场景。这里的含义是帮助一个 单所有权或单主 obj完成对body的签名（如果本地能得到signer)
    //      现在没对obj是否符合上述有权类型约束进行判断
    pub async fn sign_and_set_body<S>(
        signer: &S,
        obj: &mut AnyNamedObject,
        sign_source: &SignatureSource,
    ) -> BuckyResult<()>
    where
        S: Signer,
    {
        let sign = Self::sign_body(signer, obj, sign_source).await?;
        obj.signs_mut().unwrap().set_body_sign(sign);

        Ok(())
    }
    // 这里会检查签名是否重复
    // 这里一定会分别push一个desc的签名和body的签名！需要单独增加签名的需求要使用下边两个helper函数
    pub async fn sign_and_push<S>(
        signer: &S,
        obj: &mut AnyNamedObject,
        sign_source: &SignatureSource,
    ) -> BuckyResult<()>
    where
        S: Signer,
    {
        Self::sign_and_push_desc(signer, obj, sign_source).await?;

        if obj.has_body()? {
            Self::sign_and_push_body(signer, obj, sign_source).await?;
        }

        Ok(())
    }
    // 这里会检查签名是否重复
    pub async fn sign_and_push_desc<S>(
        signer: &S,
        obj: &mut AnyNamedObject,
        sign_source: &SignatureSource,
    ) -> BuckyResult<()>
    where
        S: Signer,
    {
        let body_sign = Self::sign_desc(signer, obj, sign_source).await?;
        obj.signs_mut().unwrap().push_desc_sign(body_sign);

        Ok(())
    }
    // 这里会检查签名是否重复
    pub async fn sign_and_push_body<S>(
        signer: &S,
        obj: &mut AnyNamedObject,
        sign_source: &SignatureSource,
    ) -> BuckyResult<()>
    where
        S: Signer,
    {
        let body_sign = Self::sign_body(signer, obj, sign_source).await?;
        obj.signs_mut().unwrap().push_body_sign(body_sign);

        Ok(())
    }

    pub async fn sign_desc<S>(
        signer: &S,
        obj: &AnyNamedObject,
        sign_source: &SignatureSource,
    ) -> BuckyResult<Signature>
    where
        S: Signer,
    {
        let hash_value = obj.desc_hash()?;
        signer.sign(hash_value.as_slice(), sign_source).await
    }

    pub async fn sign_body<S>(
        signer: &S,
        obj: &AnyNamedObject,
        sign_source: &SignatureSource,
    ) -> BuckyResult<Signature>
    where
        S: Signer,
    {
        match obj.body_hash()? {
            Some(hash_value) => signer.sign(hash_value.as_slice(), sign_source).await,
            None => {
                let msg = format!("object has no body: {}", obj.calculate_id());
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
        }
    }
}
