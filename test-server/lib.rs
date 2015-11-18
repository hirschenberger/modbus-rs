#[macro_use]
extern crate lazy_static;
use std::process::Child;
use std::sync::atomic::{AtomicUsize, Ordering};

// global unique portnumber between all test threads
lazy_static!{ static ref PORT: AtomicUsize = AtomicUsize::new(22222); }

pub struct ChildKiller(Child);

impl Drop for ChildKiller {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

pub fn start_dummy_server() -> (ChildKiller, u16) {
    use std::process::{Command, Stdio};
    use std::thread::sleep;
    use std::time::Duration;

    // get and increment global port number for current test
    let port = PORT.fetch_add(1, Ordering::SeqCst);
    let ck = ChildKiller(Command::new("./test-server/test-server")
                             .arg(port.to_string())
                             .stdout(Stdio::null())
                             .spawn()
                             .unwrap_or_else(|e| panic!("failed to execute process: {}", e)));
    sleep(Duration::from_millis(500));
    (ck, port as u16)
}
