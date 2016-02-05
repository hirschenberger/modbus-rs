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

pub fn start_dummy_server(port: Option<u16>) -> (ChildKiller, u16) {
    use std::process::{Command, Stdio};
    use std::thread::sleep;
    use std::time::Duration;

    // get and increment global port number for current test
    let p =  match port {
        Some(p) => p,
        None => PORT.fetch_add(1, Ordering::SeqCst) as u16
    };
    let ck = ChildKiller(Command::new("./test-server/test-server")
                             .arg(p.to_string())
                             .stdout(Stdio::null())
                             .spawn()
                             .unwrap_or_else(|e| panic!("failed to execute process: {}", e)));
    sleep(Duration::from_millis(500));
    (ck, p)
}
