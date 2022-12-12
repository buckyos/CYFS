
use std::cmp::Ordering;
use std::borrow::Cow;

pub struct GlobalStatePathHelper;

impl GlobalStatePathHelper {
    pub fn fix_path<'a>(path: &'a str) -> Cow<'a, str> {
        let path = path.trim();

        let ret = match path.ends_with("/") {
            true => {
                if path.starts_with('/') {
                    Cow::Borrowed(path)
                } else {
                    Cow::Owned(format!("/{}", path))
                }
            }
            false => {
                if path.starts_with('/') {
                    Cow::Owned(format!("{}/", path))
                } else {
                    Cow::Owned(format!("/{}/", path))
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