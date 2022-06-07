use std::collections::HashMap;

use cyfs_base::*;
use cyfs_core::*;
use cyfs_debug::Mutex;

use std::sync::Arc;

// 尚未评级的rank
pub const OBJECT_RANK_NONE: u8 = 0;

// 需要同步的rank
pub const OBJECT_RANK_SYNC_LEVEL: u8 = 60;

pub struct ObjectRankData {
    pub object: Arc<AnyNamedObject>,
    // object_raw: Vec<u8>,
}

struct ObjectRankStore {
    fix_rank_list: HashMap<u16, u8>,
}

impl ObjectRankStore {
    pub fn new() -> Self {
        let mut ret = Self {
            fix_rank_list: HashMap::new(),
        };

        ret.init_default();

        ret
    }

    fn init_default(&mut self) {
        // 目前zone对象需要同步？ 以ood为准也没问题
        self.fix_rank_list
            .insert(CoreObjectType::Zone.into(), OBJECT_RANK_SYNC_LEVEL);

        // storage不需要同步
        self.fix_rank_list
            .insert(CoreObjectType::Storage.into(), 10);
    }

    fn get_rank(&self, object_type: u16) -> Option<u8> {
        self.fix_rank_list.get(&object_type).map(|v| v.to_owned())
    }
}

#[derive(Clone)]
pub struct ObjectRankScorer {
    // 固定的rank
    store: Arc<Mutex<ObjectRankStore>>,
}

impl ObjectRankScorer {
    pub fn new() -> Self {
        Self {
            store: Arc::new(Mutex::new(ObjectRankStore::new())),
        }
    }

    pub fn get_rank(&self, data: &ObjectRankData) -> u8 {
        let rank = self.get_rank_impl(data);

        info!("rank object: {}, rank={}", data.object.calculate_id(), rank);

        rank
    }

    pub fn get_rank_impl(&self, data: &ObjectRankData) -> u8 {
        let object_type = data.object.obj_type();
        if let Some(rank) = self.store.lock().unwrap().get_rank(object_type) {
            return rank;
        }

        let type_code = data.object.obj_type_code();
        if type_code != ObjectTypeCode::Custom {
            self.rank_standard(type_code, data)
        } else if object_type_helper::is_core_object(object_type) {
            self.rank_core(data)
        } else if object_type_helper::is_dec_app_object(object_type) {
            self.rank_dec_app(data)
        } else {
            unreachable!();
        }
    }

    fn rank_standard(&self, type_code: ObjectTypeCode, _data: &ObjectRankData) -> u8 {
        match type_code {
            ObjectTypeCode::Device | ObjectTypeCode::People | ObjectTypeCode::SimpleGroup => 90,

            // objectmap暂时不需要同步
            ObjectTypeCode::ObjectMap => 50,
            ObjectTypeCode::Custom => {
                unreachable!();
            }
            _ => {
                // TODO 判断有没有签名
                OBJECT_RANK_SYNC_LEVEL
            }
        }
    }

    fn rank_core(&self, _data: &ObjectRankData) -> u8 {
        OBJECT_RANK_SYNC_LEVEL
    }

    fn rank_dec_app(&self, _data: &ObjectRankData) -> u8 {
        OBJECT_RANK_SYNC_LEVEL
    }
}

// FIXME 目前暂时使用全局变量
lazy_static::lazy_static! {
    pub static ref OBJECT_RANK_SCORER: ObjectRankScorer = ObjectRankScorer::new();
}
