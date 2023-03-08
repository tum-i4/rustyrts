use fork::fork;
use fork::Fork;
use libc::c_int;
use libc::waitpid;
use libc::WEXITSTATUS;
use libc::WIFEXITED;
use std::process::exit;
use std::process::Command;

fn main() {}

#[test]
fn test_output() {
    let output = Command::new("echo")
        .arg("Hello world")
        .output()
        .expect("Failed to execute command");

    assert_eq!(b"Hello world\n", output.stdout.as_slice());
}

#[test]
fn test_fork() {
    match fork() {
        Ok(Fork::Parent(child)) => {
            println!(
                "Continuing execution in parent process, new child has pid: {}",
                child
            );
        }
        Ok(Fork::Child) => {
            println!("I'm a new child process");
        }
        Err(_) => println!("Fork failed"),
    }
}
