use std::{collections::HashMap, sync::Arc};

use async_std::sync::RwLock;
use cyfs_base::{
    AnyNamedObject, BuckyError, BuckyErrorCode, BuckyResult, Group, GroupDesc, NamedObject,
    ObjectDesc, ObjectId, ObjectMapRootManagerRef, OpEnvPathAccess, RawConvertTo, RawDecode,
    RawFrom, TypelessCoreObject,
};
use cyfs_base_meta::SavedMetaObject;
use cyfs_core::{DecApp, DecAppObj, GroupShell, ToGroupShell};
use cyfs_lib::{GlobalStateManagerRawProcessorRef, NONObjectInfo};
use cyfs_meta_lib::MetaClient;

use crate::{GroupShellStatePath, MetaClientTimeout, NONDriverHelper};

const ACCESS: Option<OpEnvPathAccess> = None;

struct GroupShellCache {
    latest_shell_id: ObjectId,
    groups_by_shell: HashMap<ObjectId, Arc<Group>>,
    groups_by_version: HashMap<u64, Arc<Group>>,
}

struct GroupShellManagerRaw {
    cache: RwLock<GroupShellCache>,
    group_desc: GroupDesc,
    shell_state: ObjectMapRootManagerRef,
    state_path: GroupShellStatePath,
    meta_client: Arc<MetaClient>,
    non_driver: NONDriverHelper,
}

#[derive(Clone)]
pub struct GroupShellManager(Arc<GroupShellManagerRaw>);

impl GroupShellManager {
    pub(crate) async fn create(
        group_id: &ObjectId,
        non_driver: NONDriverHelper,
        meta_client: Arc<MetaClient>,
        local_device_id: ObjectId,
        root_state_mgr: &GlobalStateManagerRawProcessorRef,
        remote: Option<&ObjectId>,
    ) -> BuckyResult<GroupShellManager> {
        let (group, shell_id) = Self::get_group_impl(
            &non_driver,
            &meta_client,
            group_id,
            None,
            remote,
            None,
            None,
        )
        .await?;

        let shell_dec_id = Self::shell_dec_id(group_id);

        let group_state = root_state_mgr
            .load_root_state(group_id, Some(group_id.clone()), true)
            .await?
            .expect("create group-shell state failed.");

        let shell_state = group_state
            .get_dec_root_manager(&shell_dec_id, true)
            .await?;

        let group_version = group.version();
        let group = Arc::new(group);
        let raw = GroupShellManagerRaw {
            cache: RwLock::new(GroupShellCache {
                latest_shell_id: shell_id,
                groups_by_shell: HashMap::from([(shell_id.clone(), group.clone())]),
                groups_by_version: HashMap::from([(group.version(), group.clone())]),
            }),
            shell_state,
            meta_client,
            non_driver,
            state_path: GroupShellStatePath::new(),
            group_desc: group.desc().clone(),
        };

        let ret = Self(Arc::new(raw));
        Self::mount_shell(
            &ret.0.shell_state,
            &ret.0.state_path,
            &shell_id,
            group_version,
            &None,
            true,
        )
        .await?;

        Ok(ret)
    }

    pub(crate) async fn load(
        group_id: &ObjectId,
        non_driver: NONDriverHelper,
        meta_client: Arc<MetaClient>,
        local_device_id: ObjectId,
        root_state_mgr: &GlobalStateManagerRawProcessorRef,
    ) -> BuckyResult<GroupShellManager> {
        let shell_dec_id = Self::shell_dec_id(group_id);

        let group_state = root_state_mgr
            .load_root_state(group_id, Some(group_id.clone()), true)
            .await?
            .expect("create group-shell state failed.");

        let shell_state = group_state
            .get_dec_root_manager(&shell_dec_id, true)
            .await?;

        // load from cache
        let state_path = GroupShellStatePath::new();

        let op_env = shell_state.create_op_env(ACCESS)?;
        let latest_shell_id = op_env.get_by_path(state_path.latest()).await?;
        op_env.abort()?;

        let latest_shell_id = match latest_shell_id {
            Some(latest_shell_id) => latest_shell_id,
            None => {
                return Err(BuckyError::new(
                    BuckyErrorCode::NotFound,
                    format!("group({}) state for shell is not exist.", group_id),
                ))
            }
        };

        let (group, latest_shell_id) = match Self::get_group_impl(
            &non_driver,
            &meta_client,
            group_id,
            Some(&latest_shell_id),
            None,
            None,
            None,
        )
        .await
        {
            Ok(cache) => cache,
            Err(err) => {
                log::error!(
                    "get group({}) with shell({}) mounted to cache failed {:?}, will research it.",
                    group_id,
                    latest_shell_id,
                    err
                );
                let (group, shell_id) = Self::get_group_impl(
                    &non_driver,
                    &meta_client,
                    group_id,
                    None,
                    None,
                    None,
                    None,
                )
                .await?;

                Self::mount_shell(
                    &shell_state,
                    &state_path,
                    &shell_id,
                    group.version(),
                    &Some(latest_shell_id),
                    true,
                )
                .await?;

                (group, shell_id)
            }
        };

        let group = Arc::new(group);
        let raw = GroupShellManagerRaw {
            cache: RwLock::new(GroupShellCache {
                latest_shell_id,
                groups_by_shell: HashMap::from([(latest_shell_id.clone(), group.clone())]),
                groups_by_version: HashMap::from([(group.version(), group.clone())]),
            }),
            shell_state,
            meta_client,
            non_driver,
            state_path: GroupShellStatePath::new(),
            group_desc: group.desc().clone(),
        };

        let ret = Self(Arc::new(raw));

        Ok(ret)
    }

    /// get latest group in cache without query.
    /// let (latest_group, latest_shell_id) = self.group();
    pub fn group(&self) -> (Group, ObjectId) {
        async_std::task::block_on(async move {
            let cache = self.0.cache.read().await;
            let group = cache
                .groups_by_shell
                .get(&cache.latest_shell_id)
                .expect("lastest group must be exists.");
            (group.as_ref().clone(), cache.latest_shell_id.clone())
        })
    }

    pub async fn get_group(
        &self,
        group_id: &ObjectId,
        group_shell_id: Option<&ObjectId>,
        from: Option<&ObjectId>,
    ) -> BuckyResult<Group> {
        let latest_shell_id = {
            let cache = self.0.cache.read().await;
            if let Some(shell_id) = group_shell_id.as_ref() {
                let group = cache.groups_by_shell.get(*shell_id);
                if let Some(group) = group {
                    return Ok(group.as_ref().clone());
                }
            }
            cache.latest_shell_id
        };

        let (group, shell_id) = Self::get_group_impl(
            &self.0.non_driver,
            &self.0.meta_client,
            group_id,
            group_shell_id,
            from,
            Some(&latest_shell_id),
            Some(&self.0.group_desc),
        )
        .await?;

        Self::mount_shell(
            &self.0.shell_state,
            &self.0.state_path,
            &shell_id,
            group.version(),
            &Some(latest_shell_id),
            group_shell_id.is_none(),
        )
        .await?;

        {
            let mut cache = self.0.cache.write().await;
            let cached_group = Arc::new(group.clone());
            if cache
                .groups_by_shell
                .insert(shell_id, cached_group.clone())
                .is_none()
            {
                cache
                    .groups_by_version
                    .insert(group.version(), cached_group.clone());
            }
        }

        Ok(group)
    }

    async fn mount_shell(
        shell_state: &ObjectMapRootManagerRef,
        state_path: &GroupShellStatePath,
        shell_id: &ObjectId,
        version: u64,
        prev_latest_shell_id: &Option<ObjectId>,
        is_latest: bool,
    ) -> BuckyResult<()> {
        let op_env = shell_state.create_op_env(ACCESS)?;

        let version_path = state_path.version(version);
        op_env
            .set_with_path(version_path.as_str(), shell_id, &None, true)
            .await?;

        if is_latest {
            match prev_latest_shell_id {
                Some(prev_latest_shell_id) => {
                    if prev_latest_shell_id != shell_id {
                        op_env
                            .set_with_path(
                                state_path.latest(),
                                shell_id,
                                &Some(*prev_latest_shell_id),
                                false,
                            )
                            .await?;
                    }
                }
                None => {
                    op_env
                        .insert_with_path(state_path.latest(), shell_id)
                        .await?;
                }
            }
        }

        op_env.commit().await.map(|_| ())
    }

    async fn get_group_impl(
        non_driver: &NONDriverHelper,
        meta_client: &MetaClient,
        group_id: &ObjectId,
        group_shell_id: Option<&ObjectId>,
        from: Option<&ObjectId>,
        latest_group_shell_id: Option<&ObjectId>,
        group_desc: Option<&GroupDesc>,
    ) -> BuckyResult<(Group, ObjectId)> {
        match group_shell_id {
            Some(group_shell_id) => {
                let shell = non_driver.get_object(group_shell_id, from).await?;
                let (group_shell, remain) = GroupShell::raw_decode(shell.object_raw.as_slice())?;
                assert_eq!(remain.len(), 0);
                let group = if !group_shell.with_full_desc() {
                    match group_desc {
                        Some(group_desc) => group_shell.try_into_object(Some(group_desc))?,
                        None => {
                            let group = non_driver.get_object(group_id, from).await?;
                            let (group, _remain) = Group::raw_decode(group.object_raw.as_slice())?;
                            group_shell.try_into_object(Some(group.desc()))?
                        }
                    }
                } else {
                    group_shell.try_into_object(None)?
                };

                let body_hash = group.body().as_ref().unwrap().calculate_hash()?;
                // TODO: 用`body_hash`从链上验证其合法性
                let group_id_from_shell = group.desc().object_id();
                if &group_id_from_shell == group_id {
                    Ok((group, group_shell_id.clone()))
                } else {
                    let msg = format!(
                        "groupid({}) from GroupShell unmatch with the original group({})",
                        group_id_from_shell, group_id
                    );
                    log::warn!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::Unmatch, msg))
                }
            }
            None => {
                let group = meta_client.get_desc(group_id).await?;
                if let SavedMetaObject::Group(group) = group {
                    let group_shell = group.to_shell();
                    let shell_id = group_shell.shell_id();
                    if latest_group_shell_id != Some(&shell_id) {
                        // put to noc
                        let buf = group_shell.to_vec()?;
                        let shell_any = Arc::new(AnyNamedObject::Core(
                            TypelessCoreObject::clone_from_slice(buf.as_slice()).unwrap(),
                        ));
                        let shell_obj =
                            NONObjectInfo::new(shell_id, group_shell.to_vec()?, Some(shell_any));
                        non_driver.put_object(shell_obj).await?;
                    }

                    Ok((group, shell_id))
                } else {
                    let msg = format!("Object({}) from MetaChain is not a group", group_id);
                    log::warn!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::Unmatch, msg))
                }
            }
        }
    }

    fn shell_dec_id(group_id: &ObjectId) -> ObjectId {
        DecApp::generate_id(group_id.clone(), "shell")
    }
}
