use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::time::{Duration, Instant};

fn read_line(f: &mut impl Read) -> Option<String> {
    let mut buf = Vec::new();
    let mut b = [0u8; 1];
    let start = Instant::now();
    loop {
        match f.read(&mut b) {
            Ok(0) => {
                return if buf.is_empty() {
                    None
                } else {
                    Some(String::from_utf8_lossy(&buf).into())
                }
            }
            Ok(_) => {
                buf.push(b[0]);
                if b[0] == b'\n' {
                    return Some(String::from_utf8_lossy(&buf).into());
                }
            }
            Err(_) => return None,
        }
        if start.elapsed() > Duration::from_secs(5) {
            return None;
        }
    }
}

fn main() {
    let sock = std::env::var("APPDATA").unwrap() + r"\herdr\herdr.sock";
    let pipe = format!(r"\\.\pipe\{sock}");
    let mut f = OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(&pipe)
        .expect("open");
    let subs: Vec<&str> = vec![
        "workspace.created",
        "workspace.updated",
        "workspace.closed",
        "workspace.focused",
        "workspace.renamed",
        "workspace.moved",
        "tab.created",
        "tab.closed",
        "tab.focused",
        "tab.renamed",
        "tab.moved",
        "pane.created",
        "pane.closed",
        "pane.focused",
        "pane.moved",
        "pane.exited",
        "pane.agent_detected",
        "layout.updated",
        "worktree.created",
        "worktree.opened",
        "worktree.removed",
    ];
    let arr: Vec<String> = subs
        .iter()
        .map(|t| format!(r#"{{"type":"{t}"}}"#))
        .collect();
    let req = format!(
        r#"{{"id":"sub1","method":"events.subscribe","params":{{"subscriptions":[{}]}}}}"#,
        arr.join(",")
    );
    println!("REQ len {}", req.len());
    f.write_all(req.as_bytes()).unwrap();
    f.write_all(b"\n").unwrap();
    f.flush().unwrap();
    if let Some(l) = read_line(&mut f) {
        println!("ACK: {l}");
    }
    // create then close workspace to generate events
    let _ = std::process::Command::new("herdr")
        .args(["workspace", "create", "--label", "acex-evt"])
        .status();
    for i in 0..5 {
        match read_line(&mut f) {
            Some(l) => println!("EVT{i}: {l}"),
            None => {
                println!("timeout/eof at {i}");
                break;
            }
        }
    }
}
