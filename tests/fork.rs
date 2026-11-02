//! Checks that locks keep working in a child process created with `fork()`.
//!
//! This test does not use the libtest harness so that the process has exactly
//! one thread when it forks. Each case forks, drives the lock from the child
//! (and, for the shared lock, from the parent as well) and checks that the
//! child exits cleanly.
//!
//! Run a single case with `cargo test --test fork -- <case name>`.

use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use priority_inheriting_lock::{PriorityInheritingLock, SharedPriorityInheritingLock};

/// Seconds a child may run before `SIGALRM` kills it, so a regression that
/// blocks instead of panicking cannot hang the test.
const CHILD_ALARM_SECS: libc::c_uint = 5;

const PARENT_TIMEOUT: Duration = Duration::from_secs(5);

fn main() {
    let cases: [(&str, fn()); 2] = [
        ("child_thread_contention", child_thread_contention),
        ("shared_lock_across_processes", shared_lock_across_processes),
    ];
    let filter: Vec<String> = std::env::args().skip(1).collect();

    for (name, case) in cases {
        if !filter.is_empty() && !filter.iter().any(|f| f == name) {
            continue;
        }
        println!("running {name}");
        case();
        println!("{name}: ok");
    }
}

/// A child process locks, another thread in the child contends, and the first
/// thread unlocks. The kernel must agree that the child's thread owns the lock.
fn child_thread_contention() {
    static LOCK: PriorityInheritingLock<()> =
        PriorityInheritingLock::const_new(lock_api::RawMutex::INIT, ());
    static CONTENDER_STARTED: AtomicU32 = AtomicU32::new(0);

    warm_tid_cache(&LOCK);

    run_in_child(|| {
        let guard = LOCK.lock();

        let contender = thread::spawn(|| {
            CONTENDER_STARTED.store(1, Ordering::SeqCst);
            drop(LOCK.lock());
        });

        // Give the contender time to block in the kernel, so the unlock below
        // has to go through the kernel as well.
        wait_until(&CONTENDER_STARTED, 1);
        thread::sleep(Duration::from_millis(200));

        drop(guard);
        contender.join().unwrap();
    });
}

/// A process-shared lock lives in shared memory. The child takes it, and the
/// parent must then see the lock as held by someone else, not by itself.
fn shared_lock_across_processes() {
    let (lock, flag) = shared_lock_and_flag();
    warm_tid_cache(lock);

    const CHILD_LOCKED: u32 = 1;
    const PARENT_DONE: u32 = 2;

    let child = spawn_child(|| {
        let guard = lock.lock();
        flag.store(CHILD_LOCKED, Ordering::SeqCst);
        wait_until(flag, PARENT_DONE);
        drop(guard);
    });

    let deadline = Instant::now() + PARENT_TIMEOUT;
    while flag.load(Ordering::SeqCst) != CHILD_LOCKED {
        assert!(Instant::now() < deadline, "child did not take the lock");
        thread::yield_now();
    }

    assert!(
        lock.try_lock().is_none(),
        "parent acquired a lock the child holds"
    );
    flag.store(PARENT_DONE, Ordering::SeqCst);

    wait_for_child(child);

    // The child released the lock, so the parent can take it now.
    drop(lock.lock());
}

/// Caches the calling thread's id inside the crate before forking, which is
/// what a real program has done by the time it forks.
fn warm_tid_cache<R: lock_api::RawMutex>(lock: &lock_api::Mutex<R, ()>) {
    drop(lock.lock());
}

fn shared_lock_and_flag() -> (
    &'static SharedPriorityInheritingLock<()>,
    &'static AtomicU32,
) {
    let page_size = usize::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) }).unwrap();
    let page = unsafe {
        libc::mmap(
            ptr::null_mut(),
            page_size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED | libc::MAP_ANONYMOUS,
            -1,
            0,
        )
    };
    assert_ne!(page, libc::MAP_FAILED, "mmap failed");

    let lock = page.cast::<SharedPriorityInheritingLock<()>>();
    let flag = unsafe { page.cast::<u8>().add(64).cast::<AtomicU32>() };
    unsafe {
        lock.write(SharedPriorityInheritingLock::new(()));
        flag.write(AtomicU32::new(0));
        (&*lock, &*flag)
    }
}

fn wait_until(flag: &AtomicU32, value: u32) {
    while flag.load(Ordering::SeqCst) != value {
        thread::yield_now();
    }
}

fn run_in_child(f: impl FnOnce()) {
    wait_for_child(spawn_child(f));
}

/// Forks. The child runs `f` and exits with status 0. Returns the child's pid
/// to the parent.
fn spawn_child(f: impl FnOnce()) -> libc::pid_t {
    let pid = unsafe { libc::fork() };
    assert!(pid >= 0, "fork failed");
    if pid == 0 {
        unsafe { libc::alarm(CHILD_ALARM_SECS) };
        f();
        unsafe { libc::_exit(0) };
    }
    pid
}

fn wait_for_child(pid: libc::pid_t) {
    let mut status = 0;
    let waited = unsafe { libc::waitpid(pid, &mut status, 0) };
    assert_eq!(waited, pid, "waitpid failed");

    if libc::WIFEXITED(status) {
        let code = libc::WEXITSTATUS(status);
        assert_eq!(code, 0, "child exited with status {code}");
    } else if libc::WIFSIGNALED(status) {
        panic!("child was killed by signal {}", libc::WTERMSIG(status));
    } else {
        panic!("child neither exited nor was signaled, status {status}");
    }
}
