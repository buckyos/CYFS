use crate::im::{
    AddFriendDescContent, FriendOptionContent, FriendOptionDescContent, MsgDescContent,
    RemoveFriendDescContent,
};
use crate::*;
use cyfs_base::{ObjectFormat, ObjectFormatAutoWithSerde, FORMAT_FACTORY, format_json};
use serde_json::Value;

impl ObjectFormatAutoWithSerde for TextDescContent {}
impl ObjectFormatAutoWithSerde for TextContent {}

impl ObjectFormatAutoWithSerde for AppListDescContent {}
impl ObjectFormat for AppListContent {
    fn format_json(&self) -> Value {
        let mut map = serde_json::Map::new();
        for (id, status) in &self.source {
            map.insert(id.object_id().to_string(), status.format_json());
        }
        map.into()
    }
}

impl ObjectFormatAutoWithSerde for AppLocalListDesc {}
impl ObjectFormatAutoWithSerde for AppLocalListBody {}

impl ObjectFormatAutoWithSerde for AppLocalStatusDesc {}
impl ObjectFormatAutoWithSerde for AppLocalStatusBody {}

impl ObjectFormatAutoWithSerde for AppSettingDesc {}
impl ObjectFormatAutoWithSerde for AppSettingBody {}

impl ObjectFormatAutoWithSerde for AppStatusDescContent {}
impl ObjectFormatAutoWithSerde for AppStatusContent {}

impl ObjectFormatAutoWithSerde for DecAppDescContent {}
impl ObjectFormatAutoWithSerde for DecAppContent {}

impl ObjectFormatAutoWithSerde for DefaultAppListDescContent {}
impl ObjectFormatAutoWithSerde for DefaultAppListContent {}

impl ObjectFormatAutoWithSerde for AddFriendDescContent {}

impl ObjectFormatAutoWithSerde for FriendOptionDescContent {}
impl ObjectFormatAutoWithSerde for FriendOptionContent {}

impl ObjectFormatAutoWithSerde for MsgDescContent {}

impl ObjectFormatAutoWithSerde for RemoveFriendDescContent {}

impl ObjectFormat for NFTListDescContent {
    fn format_json(&self) -> Value {
        let mut array = vec![];
        for nft in &self.nft_list {
            array.push(nft.format_json())
        }
        array.into()
    }
}

impl ObjectFormatAutoWithSerde for StorageDescContent {}
impl ObjectFormat for StorageBodyContent {
    fn format_json(&self) -> Value {
        hex::encode(&self.value).into()
    }
}

impl ObjectFormatAutoWithSerde for TransContextDescContent {}
impl ObjectFormatAutoWithSerde for TransContextBodyContent {}

impl ObjectFormatAutoWithSerde for ZoneDescContent {}
impl ObjectFormatAutoWithSerde for ZoneBodyContent {}

impl ObjectFormatAutoWithSerde for FriendListDescContent {}
impl ObjectFormatAutoWithSerde for FriendContent {}

impl ObjectFormat for FriendListContent {
    fn format_json(&self) -> Value {
        let mut value = serde_json::Map::new();
        value.insert("auto_confirm".to_owned(), (self.auto_confirm == 1).into());
        value.insert("auto_msg".to_owned(), self.auto_msg.clone().into());
        let mut friends = vec![];
        for (id, _content) in &self.friends {
            friends.push(id.to_string());
        }
        value.insert("friends".to_owned(), friends.into());
        value.into()
    }
}

use std::str::FromStr;
#[test]
fn test() {
    let obj = Text::create("id1", "header1", "value1");

    let value = obj.format_json();
    let s = value.to_string();
    println!("{}", s);

    let mut friend_list = FriendList::create(ObjectId::from_str("5r4MYfFMPYJr5UqgAh2XcM4kdui5TZrhdssWpQ7XCp2y").unwrap(), true);
    friend_list.set_auto_msg("auto_msg".to_owned());
    friend_list.friend_list_mut().insert(ObjectId::from_str("5r4MYfFMPYJr5UqgAh2XcM4kdui5TZrhdssWpQ7XCp2y").unwrap(), FriendContent {});

    println!("{}", friend_list.format_json().to_string())
}


pub fn register_core_objects_format() {
    FORMAT_FACTORY.register(CoreObjectType::Zone, format_json::<Zone>);
    FORMAT_FACTORY.register(CoreObjectType::Storage, format_json::<Storage>);
    FORMAT_FACTORY.register(CoreObjectType::Text, format_json::<Text>);

    // FORMAT_FACTORY.register(CoreObjectType::FriendList, format_json::<FriendList>);

    FORMAT_FACTORY.register(CoreObjectType::TransContext, format_json::<TransContext>);
    FORMAT_FACTORY.register(CoreObjectType::DecApp, format_json::<DecApp>);
    FORMAT_FACTORY.register(CoreObjectType::AppStatus, format_json::<AppStatus>);
    FORMAT_FACTORY.register(CoreObjectType::AppList, format_json::<AppList>);
    // FORMAT_FACTORY.register(CoreObjectType::AppStoreList, format_json::<AppStoreList>);
    // FORMAT_FACTORY.register(CoreObjectType::AppExtInfo, format_json::<AppExtInfo>);

    FORMAT_FACTORY.register(CoreObjectType::DefaultAppList, format_json::<DefaultAppList>);
    
    // FORMAT_FACTORY.register(CoreObjectType::AppCmd, format_json::<AppCmd>);
    FORMAT_FACTORY.register(CoreObjectType::AppLocalStatus, format_json::<AppLocalStatus>);
    // FORMAT_FACTORY.register(CoreObjectType::AppCmdList, format_json::<AppCmdList>);
    FORMAT_FACTORY.register(CoreObjectType::AppSetting, format_json::<AppSetting>);
    // FORMAT_FACTORY.register(CoreObjectType::AppManagerAction, format_json::<AppManagerAction>);
    FORMAT_FACTORY.register(CoreObjectType::AppLocalList, format_json::<AppLocalList>);

    FORMAT_FACTORY.register(CoreObjectType::NFTList, format_json::<NFTList>);

    FORMAT_FACTORY.register(CoreObjectType::AddFriend, format_json::<AddFriend>);
    FORMAT_FACTORY.register(CoreObjectType::FriendOption, format_json::<FriendOption>);
    FORMAT_FACTORY.register(CoreObjectType::Msg, format_json::<Msg>);
    FORMAT_FACTORY.register(CoreObjectType::RemoveFriend, format_json::<RemoveFriend>);
}
