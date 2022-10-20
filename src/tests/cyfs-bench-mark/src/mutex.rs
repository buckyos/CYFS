
pub fn test() {
    let count = 1024 * 1024;
    test_std_mutex(count);
    test_cyfs_debug_mutex(count);
}

pub fn test_std_mutex(count: usize) {
    let value = std::sync::Mutex::new(0);

    println!("begin test std::sync::Mutex...");
    let begin = std::time::Instant::now();

    for _ in 0..count {
        let mut slot = value.lock().unwrap();
        *slot += 1;
    }

    let dur = begin.elapsed();
    println!("end test std::sync::Mutex: {:?}", dur);
}

pub fn test_cyfs_debug_mutex(count: usize) {
    let value = cyfs_debug::Mutex::new(0);

    println!("begin test cyfs_debug::Mutex...");
    let begin = std::time::Instant::now();

    for _ in 0..count {
        let mut slot = value.lock().unwrap();
        *slot += 1;
    }

    let dur = begin.elapsed();
    println!("end test cyfs_debug::Mutex: {:?}", dur);
}