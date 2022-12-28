mod processor;
mod transform;

pub(crate) use processor::*;
pub(crate) use transform::*;

use cyfs_base::ObjectId;

pub struct TaskGroupHelper;

impl TaskGroupHelper {
    pub fn new_opt_with_dec(dec_id: &ObjectId, group: Option<&str>) -> Option<String> {
        match group {
            Some(v) => Some(Self::new_with_dec(dec_id, v)),
            None => Some(format!("{}/", dec_id)),
        }
    }

    pub fn new_with_dec(dec_id: &ObjectId, group: &str) -> String {
        format!("{}/{}", dec_id, group.trim_start_matches('/'))
    }

    pub fn check_and_fix(dec_id: &ObjectId, group: String) -> String {
        let dec_id_str = dec_id.to_string();
        let v = group.trim_start_matches('/');
        let valid = match v.starts_with(&dec_id_str) {
            true => {
                if v.len() > dec_id_str.len() {
                    v.as_bytes()[dec_id_str.len()] as char == '/'
                } else {
                    false
                }
            }
            false => false,
        };

        if valid {
            group
        } else {
            format!("{}/{}", dec_id_str, v)
        }
    }
}


#[cfg(test)]
mod exp_tests {
    use super::*;

    #[test]
    fn test() {
        let dec_id = cyfs_core::get_system_dec_app();
        let group = format!("{}/test", dec_id);
        let group2 = TaskGroupHelper::check_and_fix(dec_id, group.clone());
        println!("{}", group2);
        assert_eq!(group, group2);

        let group3 = format!("test");
        let group4 = TaskGroupHelper::check_and_fix(dec_id, group3.clone());
        println!("{}", group4);
        assert_eq!(group, group4);

        let group5 = format!("{}_test", dec_id);
        let group6 = TaskGroupHelper::check_and_fix(dec_id, group5.clone());
        println!("{}", group6);
        assert_ne!(group5, group6);

        let group = format!("{}/", dec_id);
        let groupr = TaskGroupHelper::check_and_fix(dec_id, "/".to_owned());
        println!("{}", groupr);
        assert_eq!(group, groupr);
    }
}