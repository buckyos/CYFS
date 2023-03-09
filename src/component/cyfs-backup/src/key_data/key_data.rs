use crate::meta::KeyDataType;


#[derive(Clone, Debug)]
pub struct KeyData {
    pub local_path: String,
    pub data_type: KeyDataType,
}

impl KeyData {
    pub fn new_file(path: &str) -> Self {
        Self {
            local_path: path.to_owned(),
            data_type: KeyDataType::File,
        }
    }

    pub fn new_dir(path: &str) -> Self {
        Self {
            local_path: path.to_owned(),
            data_type: KeyDataType::Dir,
        }
    }
}

pub struct KeyDataManager {
    pub list: Vec<KeyData>,
}

impl KeyDataManager {
    pub fn new_uni() -> Self {
        let mut list = vec![];
        let data = KeyData::new_dir("etc");
        list.push(data);

        let data = KeyData::new_file("data/named-object-cache/meta.db");
        list.push(data);

        let data = KeyData::new_file("data/named-data-cache/data.db");
        list.push(data);

        let data = KeyData::new_file("data/task-manager/data.db");
        list.push(data);

        let data = KeyData::new_file("data/tracker-cache/data.db");
        list.push(data);

        let data = KeyData::new_file("data/tracker-cache/trans.db");
        list.push(data);

        let data = KeyData::new_file("data/chunk-cache/default/cache.meta");
        list.push(data);

        Self {
            list,
        }
    }
}