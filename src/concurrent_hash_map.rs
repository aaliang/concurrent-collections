use std::hash::{SipHasher, Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;
use std::{ptr, mem};
use std::collections::LinkedList;

pub struct ConcurrentHashMap <K, V> where K: Hash + PartialEq + Clone, V: Clone {
    // for now there are 16 segments
    segments: [Segment<K, V>; 16]
}

impl <K, V> ConcurrentHashMap <K, V> where K: Hash + PartialEq + Clone, V: Clone {
    pub fn new () -> ConcurrentHashMap <K, V> {
        let seg = unsafe {
            let mut seg:[Segment<K,V>; 16] = mem::uninitialized();
            for i in seg.iter_mut() {
                ptr::write(i, Segment::new());
            }
            seg
        };
        ConcurrentHashMap {
            segments: seg
        }
    }

    /// gets the key from this hashtable if it exists
    pub fn get (&self, key: &K) -> Option<&V> {
        let hash = Self::make_hash(key);
        let segment = self.segment_index(hash);
        self.segments[segment].get(key, hash)
    }

    /// Beware! do you really want to use this method?
    /// this is terribly unsafe. returning a mutable reference from here allows bypassing of the rwlock which is extremely dangerous in multithreaded use. use extremely sparingly
    /// if possible use ConcurrentHashMap::get_modify
    pub fn get_mut(&self, key: &K) -> Option<&mut V> {
        let hash = Self::make_hash(key);
        let segment = self.segment_index(hash);
        self.segments[segment].get_mut(key, hash)
    }

    /// Searches for key in the hashmap. if a value exists, calls func on the value yielding a new value which is replaced in the same transaction
    pub fn get_modify <F> (&self, key: &K, func: F) -> Option<&V> where F: Fn(&V) -> V {
        let hash = Self::make_hash(key);
        let segment = self.segment_index(hash);
        self.segments[segment].get_modify(key, hash, func)
    }

    pub fn insert (&self, key: K, val: V) {
        let hash = Self::make_hash(&key);
        let segment = self.segment_index(hash);
        self.segments[segment].insert(key, hash, val);
    }

    /// gets the segment number given a hash
    fn segment_index (&self, hash: usize) -> usize {
        hash >> 4 & (self.segments.len() - 1)
    }

    fn make_hash (key: &K) -> usize {
        let mut s = SipHasher::new();
        key.hash(&mut s);
        s.finish() as usize
    }
}

/// this is silly and awful. for small enough lists < 64 items we just use a stack allocated array
/// kind of pointless given that the LinkedLists are heap allocated anyways and we use them for each bucket.
enum InlineVec <T> {
    Static(usize, [LinkedList<T>; 64]),
    Dynamic(Vec<LinkedList<T>>)
}

/// A Segment is a write locked subset of a hashmap. Multiple reads can be done concurrently on a single segment.
pub struct Segment <K, V> where K: Hash + PartialEq + Clone, V: Clone{
    count: AtomicUsize,
    table: RwLock<InlineVec<HashEntry<K, V>>>,
    capacity: usize
}

impl <K, V> Segment <K, V> where K: Hash + PartialEq + Clone, V: Clone {
    pub fn new () -> Segment <K, V> {
        let stat = unsafe {
            let mut stat:[LinkedList<HashEntry<K, V>>; 64] = mem::uninitialized();
            for i in stat.iter_mut() {
                ptr::write(i, LinkedList::new());
            }
            stat
        };
        Segment {
            count: AtomicUsize::new(0),
            table: RwLock::new(InlineVec::Static(0, stat)),
            capacity: 8
        }
    }
    pub fn insert (&self, key: K, hash: usize, val: V) {
        let mut table = self.table.write().unwrap();
        let mut tab = match *table {
            InlineVec::Static(_, ref mut arr) => &mut arr[..],
            InlineVec::Dynamic(ref mut vec) => &mut vec[..]
        };

        let index = hash & (self.capacity - 1);
        let ref mut list = tab[index];

        for i in list.iter_mut() {
            if i.hash == hash && i.key == key {
                *i = HashEntry {
                    key: key.clone(),
                    val: val.clone(),
                    hash: hash.clone()
                };
                return;
            }
        }

        list.push_back(HashEntry {
            key: key,
            val: val,
            hash: hash
        });

        self.count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn get (&self, key: &K, hash: usize) -> Option<&V> {
        let count = self.count.load(Ordering::SeqCst);
        if count == 0 {
            None
        } else {
            let read = self.table.read().unwrap();
            let tab = match *read {
                InlineVec::Static(_, ref arr) => (&arr[..]),
                InlineVec::Dynamic(ref vec) => (&vec[..])
            };
            let index = hash & (self.capacity - 1);
            match tab[index].iter().find(|e| e.key == *key && e.hash == hash) {
                None => None,
                Some(s) => {
                    let as_raw_ptr = &s.val as *const V;
                    //return some questionably unsafe dereferenced raw pointer
                    Some(unsafe{&*as_raw_ptr})
                }
            }
        }
    }

    //see ConcurrentHashMap::get_mut above. you probably don't want to use this
    pub fn get_mut (&self, key: &K, hash: usize) -> Option<&mut V> {
        let count = self.count.load(Ordering::SeqCst);
        if count == 0 {
            None
        } else {
            let mut write = self.table.write().unwrap();
            let mut tab = match *write {
                InlineVec::Static(_, ref mut arr) => &mut arr[..],
                InlineVec::Dynamic(ref mut vec) => &mut vec[..]
            };
            let index = hash & (self.capacity - 1);
            match tab[index].iter_mut().find(|e| e.key == *key && e.hash == hash) {
                None => None,
                Some(s) => {
                    let as_raw_ptr = &mut s.val as *mut V;
                    Some(unsafe{&mut *as_raw_ptr})
                }
            }
        }
    }

    pub fn get_modify <F> (&self, key: &K, hash: usize, func: F) -> Option<&V> where F: Fn(&V) -> V {
        let count = self.count.load(Ordering::SeqCst);
        if count == 0 {
            None
        } else {
            let mut write = self.table.write().unwrap();
            let mut tab = match *write {
                InlineVec::Static(_, ref mut arr) => &mut arr[..],
                InlineVec::Dynamic(ref mut vec) => &mut vec[..]
            };
            let index = hash & (self.capacity - 1);
            match tab[index].iter_mut().find(|e| e.key == *key && e.hash == hash) {
                None => None,
                Some(s) =>  {
                    let new_val = func(&s.val);
                    *s = HashEntry {
                        hash: hash,
                        key: key.clone(),
                        val: new_val
                    };
                    let as_raw_ptr = &mut s.val as *mut V;
                    Some(unsafe{&mut *as_raw_ptr})
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct HashEntry <K, V> where K: Hash + Clone, V: Clone {
    key: K,
    val: V,
    hash: usize
}
