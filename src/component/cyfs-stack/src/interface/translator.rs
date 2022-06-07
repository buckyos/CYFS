use crate::app::AppService;
use crate::name::*;
use crate::resolver::OodResolver;
use crate::zone::ZoneManager;
use cyfs_base::*;

use http_types::{Method, Url};
use cyfs_lib::*;
use std::str::FromStr;
use std::sync::Arc;

const PATH_SEGMENT_MIN_LEN: usize = CYFS_NAME_MAX_LENGTH;
const PATH_SEGMENT_MAX_LEN: usize = 50;

enum SegItem {
    Name(String),
    Object(ObjectId),
}

struct UrlTransaltorInner {
    known_roots: Vec<&'static str>,
    name_resolver: NameResolver,
    app_service: AppService,
    zone_manager: ZoneManager,
    ood_resolver: OodResolver,
}

impl UrlTransaltorInner {
    pub fn new(
        name_resolver: NameResolver,
        app_service: AppService,
        zone_manager: ZoneManager,
        ood_resolver: OodResolver,
    ) -> Self {
        let known_roots = vec![
            "handler",
            "non",
            "ndn",
            "crypto",
            "util",
            "sync",
            "trans",
            "root-state",
            "local-cache",
        ];

        Self {
            known_roots,
            name_resolver,
            app_service,
            zone_manager,
            ood_resolver,
        }
    }

    fn is_known_roots(&self, seg: &str) -> bool {
        self.known_roots.iter().find(|v| **v == seg).is_some()
    }

    // 直接解析一个seg是不是object_id
    fn parse_seg(&self, seg: &str) -> Option<ObjectId> {
        // 只对合适的字符串才尝试解析是不是object
        // TODO 进一步优化
        if seg.len() >= PATH_SEGMENT_MIN_LEN && seg.len() <= PATH_SEGMENT_MAX_LEN {
            match ObjectId::from_str(seg) {
                Ok(id) => {
                    return Some(id);
                }
                Err(_) => {
                    // 作为name再次解析
                }
            };
        }

        None
    }

    // 解析seg列表，seg存在下面两种情况
    // 1. 编码后的object_id
    // 2. object_id对应的name
    async fn resolve_segs(&self, seg: &str) -> BuckyResult<Vec<ObjectId>> {
        let mut result = Vec::new();

        let items: Vec<&str> = seg.split(',').collect();
        for item in items {
            // CYFS_NAME_MAX_LENGTH为边界，大于此长度则是object_id，否则认为是name
            if item.len() > CYFS_NAME_MAX_LENGTH {
                match ObjectId::from_str(item) {
                    Ok(id) => {
                        result.push(id);
                    }
                    Err(e) => {
                        let msg = format!("invalid url seg as object_id: {}, {}", item, e);
                        error!("{}", msg);

                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                        // 作为name再次解析
                    }
                };
            } else {
                // 解析name
                let id = self.lookup_name(item).await.map_err(|e| {
                    error!("resolve as name but failed! seg={}, {}", item, e);
                    e
                })?;

                result.push(id);
            }
        }

        Ok(result)
    }

    /*
    cyfs://app/<app_name>/index.html
    从app_name找app id
    从app_id找app_local_status
    转换到
    cyfs://ndn/<app_local_status.webdir>/index.html
    */
    async fn translate_app_url(&self, segs: Vec<&str>) -> BuckyResult<String> {
        if segs.len() <= 1 {
            let msg = format!("app name or dec_id not specified: path={}", segs.join("/"));
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let name = segs[1];
        let web_dir = self.app_service.get_app_web_dir(name).await?;
        if web_dir.is_none() {
            let msg = format!("app not found: {}", name);
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let web_dir = web_dir.unwrap();
        let web_dir_str = web_dir.to_string();
        let mut new_segs = vec!["ndn", &web_dir_str];
        if segs.len() > 2 {
            new_segs.extend_from_slice(&segs[2..]);
        }

        let path = new_segs.join("/");

        Ok(path)
    }

    /*
    /r/{target}/{dec-id}/{inner-path}

    target: People/SimpleGroup/Device-id, name, $
    dec-id: DecAppId/system
    move target and dec-id to header, left /r/{inner-path}
    */
    async fn translate_global_state(
        &self,
        url: &http_types::Url,
        segs: Vec<&str>,
    ) -> BuckyResult<(String, Option<ObjectId>, Option<ObjectId>)> {
        assert!(segs[0] == "r" || segs[0] == "l");
        if segs.len() < 3 {
            let msg = format!("invalid r path segs: {}", url);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let target_seg = segs[1];
        let target = match target_seg {
            "$" => None,
            "$$" => {
                let ood_id = self
                    .zone_manager
                    .get_current_info()
                    .await?
                    .zone_device_ood_id
                    .object_id()
                    .clone();
                Some(ood_id)
            }
            _ => {
                let targets = self.resolve_segs(target_seg).await?;
                if targets.len() != 1 {
                    let msg = format!("only single target path support: {}", target_seg);
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                }

                Some(targets[0])
            }
        };

        let dec_seg = segs[2];
        let dec_id = match dec_seg {
            "system" => Some(cyfs_core::get_system_dec_app().object_id().to_owned()),
            "root" => None,
            _ => {
                match self.parse_seg(dec_seg) {
                    Some(id) => {
                        match id.obj_type_code() {
                            ObjectTypeCode::Custom => {
                                // treat as dec_id
                                Some(id)
                            }
                            code @ _ => {
                                let msg = format!(
                                    "invalid r path dec seg tpye: {}, type_code={:?}",
                                    dec_seg, code
                                );
                                error!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                            }
                        }
                    }
                    None => {
                        let msg = format!("invalid r path dec seg: {}", dec_seg);
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                }
            }
        };

        let inner_path = segs[3..].join("/");

        let root = match segs[0] {
            "r" => "root-state",
            "l" => "local-cache",
            _ => unreachable!(),
        };

        let path = format!("{}/{}", root, inner_path);

        info!(
            "translate r url path: {} -> ({}, {:?}, {:?})",
            url, path, target, dec_id
        );

        Ok((path, target, dec_id))
    }

    fn object_id_from_querys(name: &str, url: &Url) -> BuckyResult<Option<(ObjectId, String)>> {
        match url.query_pairs().find(|(x, _)| x == name) {
            Some((_, v)) => {
                let id = ObjectId::from_str(v.as_ref()).map_err(|e| {
                    let msg = format!("invalid query in url: {}={}, {}", name, v, e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidParam, msg)
                })?;

                Ok(Some((id, v.to_string())))
            }
            _ => Ok(None),
        }
    }

    fn calc_o_base(mode: &RootStateAccessGetMode, id: &ObjectId) -> &'static str {
        match mode {
            RootStateAccessGetMode::Default => match id.obj_type_code() {
                ObjectTypeCode::File | ObjectTypeCode::Dir | ObjectTypeCode::ObjectMap => "ndn",
                _ => "non",
            },
            RootStateAccessGetMode::Object => "non",
            RootStateAccessGetMode::Data => "ndn",
        }
    }

    fn mode_format_from_request(url: &http_types::Url) -> RootStateAccessGetMode {
        match RequestorHelper::value_from_querys("mode", url) {
            Ok(Some(mode)) => mode,
            Ok(None) => RootStateAccessGetMode::Default,
            Err(_) => RootStateAccessGetMode::Default,
        }
    }

    pub async fn translate_url(&self, req: &mut http_types::Request) -> BuckyResult<()> {
        // 只尝试对get请求转换url
        if req.method() != http_types::Method::Get {
            return Ok(());
        }

        let segs = match req.url().path_segments() {
            Some(it) => {
                let list: Vec<&str> = it.collect();
                if list.is_empty() {
                    return Ok(());
                }

                list
            }
            None => return Ok(()),
        };

        let root = segs[0];
        if root.is_empty() {
            return Ok(());
        }
        if self.is_known_roots(root) {
            return Ok(());
        }

        if root == "r" || root == "l" {
            let (path, target, dec_id) = self.translate_global_state(req.url(), segs).await?;
            req.url_mut().set_path(&path);

            if let Some(target) = target {
                req.append_header(cyfs_base::CYFS_TARGET, &target.to_string());
            }
            if let Some(dec_id) = dec_id {
                req.append_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
            }

            return Ok(());
        }

        if root == "app" {
            let path = self.translate_app_url(segs).await?;
            info!("translate app url path: {} -> {}", req.url().path(), path);
            req.url_mut().set_path(&path);
            return Ok(());
        }

        /*
        url范式如下：
        cyfs:://[device_id|device_list|owner/]object_id/[inner_path]
        其中第一、三段是可选，所以解析时候，需要判断第二段是不是object_id

        cyfs://$objid  -> non/$objid, target=None
        cyfs://$owner/$objid -> non/$objid, target=$owner
        cyfs://$fileid -> ndn/$fileid
        cyfs://$device_list/$fileid  -> ndn/$fileid, target=$device_list
        只有第一级支持name，第二级不支持name，会被认为是inner_path
        所有模式都可以带inner_path
        */

        let roots = self.resolve_segs(root).await?;
        let second_seg = if segs.len() >= 2 {
            self.parse_seg(segs[1])
        } else {
            None
        };

        // 查看是否明确指定了mode
        let mode: RootStateAccessGetMode = Self::mode_format_from_request(req.url());

        // 确定基础路由
        let mut target = None;
        let mut parts: Vec<&str> = vec![];
        let id_str;
        match second_seg {
            Some(id) => {
                // 二段模式
                // 第一段root就是target，可能是一个列表
                target = Some(segs[0].to_owned());

                // $owner|$device_list/$objid
                let base = Self::calc_o_base(&mode, &id);
                parts.push(base);
            }
            None => {
                // 一段模式, roots就是object_id，目前只支持一个
                if roots.len() != 1 {
                    let msg = format!("only single root path support: {}", root);
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                }

                let id = &roots[0];

                // 解析目标
                if let Ok(list) = self.resolve_target_from_object(id).await {
                    if list.len() > 0 {
                        target = Some(list[0].to_string());
                    }
                }

                let base = Self::calc_o_base(&mode, &id);
                parts.push(base);

                id_str = id.to_string();
                parts.push(id_str.as_str());
            }
        };


        /*
        let root: Vec<String> = roots.iter().map(|id| id.to_string()).collect();
        let root = root.join(",");
        parts.push(&root);
        */
        
        // 如果第一段是target，那么需要跳过
        parts.extend_from_slice(&segs[1..]);

        drop(segs);

        let path = parts.join("/");
        info!(
            "translate url path: {} -> {}, target={:?}",
            req.url().path(),
            path,
            target
        );
        req.url_mut().set_path(&path);

        // target需要放到header里面
        if let Some(target) = target {
            // 如果是get，NDN的get_data的参数在这种情况下会以query params形式提供
            if req.method() == Method::Get {
                let target_query = format!("{}={}", cyfs_base::CYFS_TARGET, target);
                let query = match req.url_mut().query() {
                    Some(v) => format!("{}&&{}", target_query, v),
                    None => target_query,
                };
                req.url_mut().set_query(Some(&query));
            }

            req.append_header(cyfs_base::CYFS_TARGET, target);
        }

        if let Some((_, dec_id)) = Self::object_id_from_querys("dec_id", req.url())? {
            req.append_header(cyfs_base::CYFS_DEC_ID, &dec_id);
        }

        Ok(())
    }

    // 解析name<->object的绑定关系
    async fn lookup_name(&self, name: &str) -> BuckyResult<ObjectId> {
        match self.name_resolver.lookup(name).await? {
            NameResult::ObjectLink(id) => Ok(id),
            NameResult::IPLink(addr) => {
                let msg = format!("name system not support iplink yet! {} -> {}", name, addr);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    async fn resolve_target_from_object(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Vec<DeviceId>> {
        let mut sources = vec![];
        match self
            .ood_resolver
            .resolve_ood(
                object_id,
                None)
            .await
        {
            Ok(list) => {
                if list.is_empty() {
                    info!(
                        "get target from path root seg but not found! seg={}",
                        object_id,
                    );
                } else {
                    info!(
                        "get target from path root seg success! seg={}, sources={:?}",
                        object_id, list
                    );

                    list.into_iter().for_each(|device_id| {
                        // 这里需要列表去重
                        if !sources.iter().any(|v| *v == device_id) {
                            sources.push(device_id);
                        }
                    });
                }

                Ok(sources)
            }
            Err(e) => {
                error!(
                    "get target from path root seg failed! id={}, {}",
                    object_id, e
                );
                Err(e)
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct UrlTransaltor(Arc<UrlTransaltorInner>);

impl UrlTransaltor {
    pub fn new(
        name_resolver: NameResolver,
        app_service: AppService,
        zone_manager: ZoneManager,
        ood_resolver: OodResolver,
    ) -> Self {
        Self(Arc::new(UrlTransaltorInner::new(
            name_resolver,
            app_service,
            zone_manager,
            ood_resolver,
        )))
    }

    pub async fn translate_url(&self, req: &mut http_types::Request) -> BuckyResult<()> {
        self.0.translate_url(req).await
    }
}
