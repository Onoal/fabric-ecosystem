#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::process;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("long-running") => {
            let marker = args.next().expect("marker path");
            let token = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos();
            fs::write(marker, format!("{}\n{token}\n", process::id())).expect("write marker");
            loop {
                thread::sleep(Duration::from_millis(100));
            }
        }
        Some("exit-success") => process::exit(0),
        Some("exit-failure") => process::exit(17),
        other => {
            eprintln!("unknown process-helper mode: {other:?}");
            process::exit(64);
        }
    }
}
