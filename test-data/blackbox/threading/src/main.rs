use std::process::exit;
use std::sync::mpsc::channel;
use std::thread;
use threadpool::ThreadPool;

// The purpose of this crate is, to demonstrate why it is necessary to execute tests in separate processes
// in dynamic RustyRTS
// The traces of test1 should not contain test2 and vice versa

fn main() {}

#[test]
fn test1() {
    // source: https://doc.rust-lang.org/rust-by-example/std_misc/threads.html

    const NTHREADS: u32 = 10;

    // Make a vector to hold the children which are spawned.
    let mut children = vec![];

    for i in 0..NTHREADS {
        // Spin up another thread
        children.push(thread::spawn(move || {
            println!("this is thread number {}", i);

            #[cfg(feature = "changes_test1")]
            println!("Unexpected");
        }));
    }

    for child in children {
        // Wait for the thread to finish. Returns a result.
        let _ = child.join();
    }

    #[cfg(feature = "test1_panic")]
    panic!();
}

#[test]
fn test2() {
    // source: https://docs.rs/threadpool/latest/threadpool/

    let n_workers = 4;
    let n_jobs = 8;
    let pool = ThreadPool::new(n_workers);

    let (tx, rx) = channel();
    for _ in 0..n_jobs {
        let tx = tx.clone();
        pool.execute(move || {
            tx.send(1)
                .expect("channel will be there waiting for the pool");

            #[cfg(feature = "changes_test2")]
            println!("Unexpected");
        });
    }

    assert_eq!(rx.iter().take(n_jobs).fold(0, |a, b| a + b), 8);

    #[cfg(feature = "test2_panic")]
    panic!();
}
