extern crate cocollections;

use cocollections::concurrent_hash_map::{Segment, ConcurrentHashMap};

//this is just a driver right now. these tests dont actually assert anything

#[test]
fn it_works() {
    let _: ConcurrentHashMap<String, i32> = ConcurrentHashMap::new();
}

#[test]
fn test_segment () {
    let _: Segment<String, i32> = Segment::new();
}

use std::thread;
use std::sync::Arc;

#[test]
fn test_multiple_thread_access () {
    let mut chm:ConcurrentHashMap<String, i32> = ConcurrentHashMap::new();
    let shared_map = Arc::new(chm);
    for _ in 0 .. 10 {
        let local = shared_map.clone();
        thread::spawn(move || {
            let something = "hello".to_string();
            local.insert(something, 3);
        });
    }
}

fn main () {
    let mut chm = ConcurrentHashMap::new();
    {
        chm.insert("hello", 3);
        let mesg = chm.get_mut(&"hello");

        if let Some(x) = mesg {
            *x = 4;
        }
    }

    println!("{:?}", chm.get(&"hello"));

    {
        chm.get_modify(&"hello", |_| 8);
    }

    println!("{:?}", chm.get(&"hello"));
    //println!("{:?}", mesg);
}
