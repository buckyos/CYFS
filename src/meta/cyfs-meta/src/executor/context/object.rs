use cyfs_base::*;
use cyfs_base_meta::*;
// pub trait FFSObject {
//     fn update_desc(self:&mut Self,opid:&ObjectId,ext_sig:&Vec<TxSig>,
//                    desc:&Desc,write_flag:u8,price:&Option<MetaPrice>) -> Result<(),u32>;

//     fn get_state(self:& Self) -> FFSObjectState;
// }



// pub fn create_obj_desc(obj_id:&ObjectId, desc:&Desc, coin_id:u8, prices:u32, init_value:u64, db:&MetaDBClient) -> Result<(),u32> {
//     unimplemented!();
// }

// pub fn get_object(obj_id:&ObjectId, db:&MetaDBClient) -> Result<Box<dyn FFSObject>,u32> {
//     unimplemented!();
// }



pub fn id_from_desc(desc: &SavedMetaObject) -> ObjectId {
    match desc {
        SavedMetaObject::Device(peer) => {
            peer.desc().calculate_id()
        },
        SavedMetaObject::People(peer) => {
            peer.desc().calculate_id()
        },
        SavedMetaObject::UnionAccount(desc) => {
            desc.desc().calculate_id()
        },
        SavedMetaObject::Group(obj) => {
            obj.desc().calculate_id()
        },
        // SavedMetaObject::AppGroupDesc(_) => {
        //     unimplemented!()
        // },
        // SavedMetaObject::OrgDesc(_) => {
        //     unimplemented!()
        // },
        SavedMetaObject::File(obj) => {
            obj.desc().calculate_id()
        },
        SavedMetaObject::Data(obj) => {
            obj.id.clone()
        },
        SavedMetaObject::MinerGroup(group) => {
            group.desc().calculate_id()
        },
        SavedMetaObject::Contract(contract) => {
            contract.desc().calculate_id()
        },
        SavedMetaObject::SNService(service) => {
            service.desc().calculate_id()
        }
        SavedMetaObject::SimpleGroup => {
            panic!("SimpleGroup is deprecated, you can use the Group.")
        }
        SavedMetaObject::Org => panic!("Org is deprecated, you can use the Group."),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetaDescObject {
    Device,
    People,
    Unkown,
}