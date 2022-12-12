
use std::cmp::Ordering;

pub struct GlobalStatePathHelper;

impl GlobalStatePathHelper {
    pub fn fix_path(path: impl Into<String> + AsRef<str>) -> String {
        let path = path.as_ref().trim();

        let ret = match path.ends_with("/") {
            true => {
                if path.starts_with('/') {
                    path.into()
                } else {
                    format!("/{}", path.as_ref() as &str)
                }
            }
            false => {
                if path.starts_with('/') {
                    format!("{}/", path.as_ref() as &str)
                } else {
                    format!("/{}/", path.as_ref() as &str)
                }
            }
        };

        ret
    }

    pub fn compare_path(left: &String, right: &String) -> Option<Ordering> {
        let len1 = left.len();
        let len2 = right.len();

        if len1 > len2 {
            Some(Ordering::Less)
        } else if len1 < len2 {
            Some(Ordering::Greater)
        } else {
            left.partial_cmp(right)
        }
    }
}