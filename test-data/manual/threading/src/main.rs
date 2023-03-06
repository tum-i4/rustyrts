use std::process::exit;
use std::sync::mpsc::channel;
use std::thread;
use threadpool::ThreadPool;

fn main() {}

// This is the `main` thread
#[test]
fn test() {
    // source: https://doc.rust-lang.org/rust-by-example/std_misc/threads.html

    const NTHREADS: u32 = 10;

    // Make a vector to hold the children which are spawned.
    let mut children = vec![];

    for i in 0..NTHREADS {
        // Spin up another thread
        children.push(thread::spawn(move || {
            println!("this is thread number {}", i);
        }));
    }

    for child in children {
        // Wait for the thread to finish. Returns a result.
        let _ = child.join();
    }
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
        });
    }

    assert_eq!(rx.iter().take(n_jobs).fold(0, |a, b| a + b), 8);
}
