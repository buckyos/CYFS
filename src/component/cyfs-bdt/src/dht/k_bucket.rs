use std::collections::VecDeque;
use std::collections::vec_deque::{Iter, IterMut};

pub trait KadId: PartialEq {
    fn compare(&self, other: &Self) -> std::cmp::Ordering;
    fn distance(&self, other: &Self) -> Self;
    fn kad_index(dist: &Self) -> u32;
    fn bits()->u32;
}

pub trait KadEntry {
    fn newest_than(&self, other: &Self) -> bool;
}

pub enum KBucketResult<E> {
    Added(Option<E>),
    Updated,
    Ignored,
}

pub struct KBucketIter<'a, T, E> {
    curr: Iter<'a, (T, E)>,
}
impl<'a, T, E> Iterator for KBucketIter<'a, T, E> {
    type Item = &'a (T, E);
    fn next(&mut self) -> Option<Self::Item> {
        self.curr.next()
    }
}

pub struct KBucketIterMut<'a, T, E> {
    curr: IterMut<'a, (T, E)>,
}
impl<'a, T, E> Iterator for KBucketIterMut<'a, T, E> {
    type Item = &'a mut (T, E);
    fn next(&mut self) -> Option<Self::Item> {
        self.curr.next()
    }
}

struct KBucket<T: KadId + Clone + PartialEq, E: KadEntry + Clone> {
    entries: VecDeque<(T, E)>,
    k_size: u32,
}

impl<T: KadId+ Clone + PartialEq, E: KadEntry + Clone> KBucket<T, E> {
    fn new(k_size: u32) -> Self {
        Self {
            entries: VecDeque::new(),
            k_size,
        }
    }

    pub fn set(&mut self, id: &T, new_entry: &E) -> KBucketResult<E> {
        for entry in self.entries.iter_mut() {
            if &entry.0 == id {
                if new_entry.newest_than(&entry.1) {
                    entry.1 = new_entry.clone();
                    return  KBucketResult::Updated;
                }
                return KBucketResult::Ignored;
            }
        }

        let result = if self.entries.len() < self.k_size as usize {
            KBucketResult::Added(None)
        } else {
            KBucketResult::Added(Some(self.entries.pop_front().unwrap().1))
        };
        self.entries.push_back((id.clone(), new_entry.clone()));

        result
    }

    pub fn get(&self, id: &T) -> Option<&E> {
        for entry in self.entries.iter() {
            if &entry.0 == id {
                return Some(&entry.1);
            }
        }

        None
    }

    pub fn get_mut(&mut self, id: &T) -> Option<&mut E> {
        for entry in self.entries.iter_mut() {
            if &entry.0 == id {
                return Some(&mut entry.1);
            }
        }

        None
    }

    pub fn iter(&self) -> KBucketIter<'_, T, E> {
        KBucketIter {curr: self.entries.iter()}
    }

    pub fn iter_mut(&mut self) -> KBucketIterMut<'_, T, E> {
        KBucketIterMut {curr: self.entries.iter_mut()}
    }

    pub fn len(&self)->usize {
        self.entries.len()
    }
}

pub struct KBuckets<T: KadId + Clone + PartialEq, E: KadEntry + Clone> {
    buckets: Vec<KBucket<T, E>>,
    owner: T,
    k_size: u32,
}
impl<T: KadId + Clone + PartialEq, E: KadEntry + Clone> KBuckets<T, E> {
    pub fn new(k_size: u32, owner: T) -> Self {
        let mut buckets = Vec::new();
        for _ in 0..T::bits() as usize {
            buckets.push(KBucket::new(k_size));
        }
        Self {
            buckets,
            owner,
            k_size,
        }
    }

    pub fn set(&mut self, id: &T, entry: &E) -> KBucketResult<E> {
        let distance = self.owner.distance(id);
        let index = T::kad_index(&distance);
        assert!(index < T::bits());
        self.buckets[index as usize].set(id, entry)
    }

    pub fn get(&self, id: &T) -> Option<&E> {
        for bucket in self.buckets.iter() {
            if let Some(e) = bucket.get(id) {
                return Some(e);
            }
        }

        None
    }

    pub fn get_mut(&mut self, id: &T) -> Option<&mut E> {
        for bucket in self.buckets.iter_mut() {
            if let Some(e) = bucket.get_mut(id) {
                return Some(e);
            }
        }

        None
    }

    pub fn get_nearest_of(&self, id: &T) -> Vec<(&T, &E)> {
        let mut nearest = Vec::new();
        for bucket in self.buckets.iter() {
            for entry in bucket.iter() {
                nearest.push((&entry.0, &entry.1));
            }
        }

        nearest.sort_by(|a, b| a.0.distance(id).compare(&b.0.distance(id)));
        nearest.truncate(self.k_size as usize);
        nearest
    }
}



impl KadId for u32 {
    fn compare(&self, _other: &Self) -> std::cmp::Ordering {
        std::cmp::Ordering::Equal
    }
    fn distance(&self, _other: &Self) -> Self {
        0
    }
    fn kad_index(_dist: &Self) -> u32 {
        0
    }
    fn bits()->u32 {
        16
    }
}
impl KadEntry for u32 {
    fn newest_than(&self, _other: &Self) -> bool {
        false
    }
}

#[test]
fn test_bucket() {
    let mut bucket = KBucket::<u32, u32>::new(10);
    let result = bucket.set(&0, &1);
    match result {
        KBucketResult::Added(old) => {
            assert!(old.is_none());
        },
        _ => {}
    };
    let _ = bucket.set(&1, &2);

    for v in bucket.iter() {
        println!("{}", v.1);
    }

    let mut buckets = KBuckets::new(10, 5);
    let result = buckets.set(&0, &1);
    match result {
        KBucketResult::Added(old) => {
            assert!(old.is_none());
        },
        _ => {}
    };
    let _result = buckets.get_nearest_of(&10);
}