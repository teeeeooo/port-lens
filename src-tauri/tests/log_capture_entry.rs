//! Exercises the actual shipped binary entry before Tauri/single-instance setup.
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[test]
fn shipped_collector_rotates_and_exits_on_eof_without_opening_gui() {
    let dir = std::env::temp_dir().join(format!(
        "port-lens-log-entry-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("stdout.log");
    let mut child = Command::new(env!("CARGO_BIN_EXE_port-lens"))
        .arg("--port-lens-log-writer")
        .arg(&path)
        .arg("\n=== entry test run ===\n")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let output = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(output).read_line(&mut line).map(|_| line);
        let _ = tx.send(result);
    });
    let ready = rx.recv_timeout(Duration::from_secs(10));
    if ready.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    assert_eq!(ready.unwrap().unwrap(), "PORT_LENS_LOG_READY\n");
    let mut input = child.stdin.take().unwrap();
    // A line much larger than a chunk, repeated over more than two generations.
    for index in 0..120 {
        let mut block = vec![b'x'; 128 * 1024];
        let marker = format!("ENTRY_BLOCK_{index:03}\n");
        let end = block.len();
        block[end - marker.len()..].copy_from_slice(marker.as_bytes());
        input.write_all(&block).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let visible = (|| -> std::io::Result<bool> {
                let mut file = fs::File::open(&path)?;
                let size = file.metadata()?.len();
                file.seek(SeekFrom::Start(size.saturating_sub(64)))?;
                let mut tail = Vec::new();
                file.read_to_end(&mut tail)?;
                Ok(tail.ends_with(marker.as_bytes()))
            })()
            .unwrap_or(false);
            if visible {
                break;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("output was not captured");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    input.write_all(b"\nFINAL-ENTRY-MARKER\n").unwrap();
    drop(input);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("collector did not exit on EOF");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let current = fs::read(&path).unwrap();
    let previous = fs::read(path.with_extension("log.1")).unwrap();
    assert!(current.len() <= 5 * 1024 * 1024);
    assert!(previous.len() <= 5 * 1024 * 1024);
    assert!(String::from_utf8_lossy(&current).contains("FINAL-ENTRY-MARKER"));
    assert!(String::from_utf8_lossy(&current).contains("entry test run"));
    assert!(!path.with_extension("log.2").exists());
    fs::remove_dir_all(dir).unwrap();
}
