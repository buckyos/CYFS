use int_enum::IntEnum;

#[derive(Clone, Eq, Copy, PartialEq, Debug, IntEnum)]
#[repr(u16)]
pub enum CoreObjectType {
    // ZONE
    Zone = 32,

    // admin control
    Admin = 33,

    // 基于object的存储
    Storage = 40,

    // 文本对象
    Text = 41,

    // 通讯录
    // FriendList = 130,
    FriendOption = 131,
    FriendProperty = 132,

    //meta相关
    BlockV1 = 300,
    MetaProto = 301,
    MetaMinerGroup = 302,
    BlockV2 = 303,

    // Trans
    TransContext = 350,
    // app相关
    DecApp = 400,
    AppStatus = 401,
    AppList = 402,
    // PutApp = 403,
    // RemoveApp = 404,

    // app相关，这两个对象现在只有在ts里实现，ts和网页中用到。rust里暂时用不到
    AppStoreList = 405,
    AppExtInfo = 406,

    // 默认app
    DefaultAppList = 407,
    SetDefaultApp = 408,

    // AppLocalStatus = 409,
    //App控制对象
    AppCmd = 410,
    //AppLocalStatusEx = 411,
    AppLocalStatus = 411,
    //AppListEx = 412,
    AppCmdList = 413,
    AppSetting = 414,
    AppManagerAction = 415,
    AppLocalList = 416,

    // 钱包相关对象
    NFTList = 500,

    // Perf
    PerfOperation = 600,

    // Group
    GroupProposal = 700,
    GroupUpdateGroup = 701,
    GroupConsensusBlock = 702,
    GroupRPathStatus = 703,
    GroupAction = 704,
    GroupQuorumCertificate = 705,

    // IM通用对象
    AddFriend = 1001,
    Msg = 1003,
    RemoveFriend = 1004,

    // 错误
    // cyfs_base::OBJECT_TYPE_CORE_END
    ErrObjType = 32767,
}

impl Into<u16> for CoreObjectType {
    fn into(self) -> u16 {
        self as u16
    }
}

impl From<u16> for CoreObjectType {
    fn from(value: u16) -> Self {
        match Self::from_int(value) {
            Ok(v) => v,
            Err(e) => {
                error!("unknown CoreObjectType value: {} {}", value, e);
                Self::ErrObjType
            }
        }
    }
}

impl CoreObjectType {
    pub fn as_u16(&self) -> u16 {
        let v: u16 = self.clone().into();
        v
    }
}
