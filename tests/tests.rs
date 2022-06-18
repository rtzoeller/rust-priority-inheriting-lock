use priority_inheriting_lock::{gettid, PriorityInheritingLock};

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

#[cfg(test)]
#[macro_export]
macro_rules! require_root {
    ($name:expr) => {
        use ::nix::unistd::Uid;
        use ::std::io::{self, Write};

        if !Uid::current().is_root() {
            // eprintln participates in test output capturing; this bypasses it to print unconditionally.
            let stderr = io::stderr();
            let mut handle = stderr.lock();
            writeln!(handle, "{} requires root privileges. Skipping test.", $name).unwrap();
            return;
        }
    };
}

#[test]
fn smoke() {
    let m = PriorityInheritingLock::new(());
    drop(m.lock());
    drop(m.lock());
}

#[test]
fn try_lock_uncontended() {
    let m = PriorityInheritingLock::new(());
    assert!(m.try_lock().is_some());
}

#[test]
fn try_lock_contended() {
    let m = Arc::new(PriorityInheritingLock::new(()));
    let m2 = m.clone();

    let _g = m.lock();
    let t = thread::spawn(move || {
        let g2 = m2.try_lock();

        return g2.is_some();
    });

    assert_eq!(false, t.join().unwrap());
}

fn set_scheduler(policy: i32, priority: i32) {
    unsafe {
        let pthread_id = libc::pthread_self();
        let param = libc::sched_param {
            sched_priority: priority,
        };

        std::thread::sleep(std::time::Duration::from_millis(10));
        // Use nix's error conversions, just because they're a convenient way to get a string representation
        // of whatever this returns on failure (probably EPERM).
        match libc::pthread_setschedparam(pthread_id, policy, &param) {
            0 => (),
            err => panic!("{}", nix::errno::Errno::from_i32(err)),
        }
    }
}

#[test]
fn priority_is_inherited() {
    require_root!("priority_is_inherited");

    let t = thread::spawn(|| {
        let m = Arc::new(PriorityInheritingLock::new(1));
        let m2 = m.clone();
        let boosted = AtomicBool::new(false);
        set_scheduler(libc::SCHED_FIFO, 30);
        let _guard = m.lock();

        let _ = thread::spawn(move || {
            set_scheduler(libc::SCHED_FIFO, 60);
            let _guard = m2.lock();
            boosted.store(true, Ordering::SeqCst);
        });

        let thread_id = gettid();
        let path = format!("/proc/{thread_id}/stat");
        let path = Path::new(&path);
        std::thread::sleep(std::time::Duration::from_millis(10));
        let mut file = File::open(&path).unwrap();
        let mut string = String::new();
        file.read_to_string(&mut string).unwrap();
        let priority_str = string.split(" ").nth(17).unwrap();
        let priority = priority_str.parse::<i32>().unwrap();
        assert_eq!(priority, -61)
    });

    t.join().unwrap();
}
