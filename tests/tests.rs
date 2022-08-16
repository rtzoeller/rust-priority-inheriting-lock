use priority_inheriting_lock::{gettid, PriorityInheritingLock};

use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use procfs::process::Process;

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

// Modeled after https://github.com/Amanieu/parking_lot/blob/336a9b31ff385728d00eb7ef173e4d054584b787/src/mutex.rs#L131
#[test]
fn smoke() {
    let m = PriorityInheritingLock::new(());
    drop(m.lock());
    drop(m.lock());
}

// Modeled after https://github.com/Amanieu/parking_lot/blob/336a9b31ff385728d00eb7ef173e4d054584b787/src/mutex.rs#L139
#[test]
fn lots_and_lots() {
    const J: u32 = 1000;
    const K: u32 = 3;

    let m = Arc::new(PriorityInheritingLock::new(0));

    fn inc(m: &PriorityInheritingLock<u32>) {
        for _ in 0..J {
            *m.lock() += 1;
        }
    }

    let (tx, rx) = channel();
    for _ in 0..K {
        let tx2 = tx.clone();
        let m2 = m.clone();
        thread::spawn(move || {
            inc(&m2);
            tx2.send(()).unwrap();
        });
        let tx2 = tx.clone();
        let m2 = m.clone();
        thread::spawn(move || {
            inc(&m2);
            tx2.send(()).unwrap();
        });
    }

    drop(tx);
    for _ in 0..2 * K {
        rx.recv().unwrap();
    }
    assert_eq!(*m.lock(), J * K * 2);
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

    let t = thread::spawn(move || m2.try_lock().is_some());

    assert!(!t.join().unwrap());
}

#[test]
fn try_lock_for_uncontended() {
    let m = PriorityInheritingLock::new(());
    assert!(m
        .try_lock_for(std::time::Duration::from_millis(10))
        .is_some());
}

#[test]
fn try_lock_for_contended() {
    let m = Arc::new(PriorityInheritingLock::new(()));
    let m2 = m.clone();
    let g = m.lock();

    let t = thread::spawn(move || {
        let mut attempts = 1;
        loop {
            match m2.try_lock_for(std::time::Duration::from_millis(10)) {
                Some(_) => return attempts,
                None => attempts += 1,
            }
        }
    });

    std::thread::sleep(std::time::Duration::from_millis(50));
    drop(g);

    let attempts = t.join().unwrap();
    assert!((4..=6).contains(&attempts));
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
        set_scheduler(libc::SCHED_FIFO, 30);
        let tid = gettid();
        let task = Process::myself().unwrap().task_from_tid(tid).unwrap();
        let original_priority = task.stat().unwrap().priority;
        assert_eq!(original_priority, -31);
        let _guard = m.lock();

        let _ = thread::spawn(move || {
            set_scheduler(libc::SCHED_FIFO, 60);
            let _guard = m2.lock();
        });

        let start = Instant::now();
        loop {
            let boosted_priority = task.stat().unwrap().priority;
            if boosted_priority == -61 {
                break;
            } else if start.elapsed().as_millis() > 100 {
                panic!("Thread's priority was not boosted within expected time.");
            } else {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
    });

    t.join().unwrap();
}
