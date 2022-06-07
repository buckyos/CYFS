use std::collections::{hash_map::Entry, HashMap};


pub trait PerfItemMerge<T> {
    fn merge(&mut self, other: T);
}

// 为HashMap实现一个统一的Merge
impl<T> PerfItemMerge<HashMap<String, T>> for HashMap<String, T>
where
    T: PerfItemMerge<T>,
{
    fn merge(&mut self, other: HashMap<String, T>) {
        for (key, value) in other {
            match self.entry(key) {
                Entry::Occupied(mut o) => o.get_mut().merge(value),
                Entry::Vacant(v) => {
                    v.insert(value);
                }
            }
        }
    }
}