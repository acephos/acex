fn main() {
    use std::fs::OpenOptions;
    use std::io::{Read, Write};
    use std::os::windows::fs::OpenOptionsExt;

    let sock = std::env::var("APPDATA").unwrap() + r"\herdr\herdr.sock";
    let pipe = format!(r"\\.\pipe\{sock}");
    println!("pipe={pipe}");
    // FILE_FLAG_OVERLAPPED = 0x40000000 — skip for sync first
    for flags in [0u32, 0x40000000] {
        println!("flags={flags:#x}");
        match OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .custom_flags(flags)
            .open(&pipe)
        {
            Ok(mut f) => {
                println!("opened");
                let msg = b"{\"id\":\"t1\",\"method\":\"ping\",\"params\":{}}\n";
                match f.write_all(msg) {
                    Ok(()) => {
                        let _ = f.flush();
                        println!("wrote ok");
                        let mut buf = vec![0u8; 8192];
                        match f.read(&mut buf) {
                            Ok(n) => println!("read {n}: {}", String::from_utf8_lossy(&buf[..n])),
                            Err(e) => println!("read err: {e}"),
                        }
                    }
                    Err(e) => println!("write err: {e}"),
                }
            }
            Err(e) => println!("open err: {e}"),
        }
    }
}
