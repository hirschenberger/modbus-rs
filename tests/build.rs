use std::process::Command;

fn main() {
    if cfg!(feature = "modbus-server-tests") {
        let pkg_cfg = Command::new("pkg-config")
                          .args(&["--libs", "--cflags", "libmodbus"])
                          .output()
                          .unwrap_or_else(|e| panic!("Error running pkg-config: {}", e));
        let output = String::from_utf8_lossy(&pkg_cfg.stdout);
        let flags: Vec<&str> = output.split_whitespace().collect();
        let out = Command::new("gcc")
                      .args(&["tests/test-server.c", "-o", "tests/test-server"])
                      .args(&flags[..])
                      .output()
                      .unwrap_or_else(|e| panic!("Error running gcc: {}", e));
        if !out.status.success() {
            panic!("Error building testserver");
        }
    }
}
