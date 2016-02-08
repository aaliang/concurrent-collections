extern crate cocollections;

use cocollections::concurrent_hash_map::{Segment, ConcurrentHashMap};

#[test]
fn it_works() {
    let _: ConcurrentHashMap<String, i32> = ConcurrentHashMap::new();
}

#[test]
fn test_segment () {
    let _: Segment<String, i32> = Segment::new();
}

fn main () {
    let mut chm = ConcurrentHashMap::new();

    chm.insert("hello", 3);
    let mesg = chm.get(&"hello");
    println!("{:?}", mesg);
}
