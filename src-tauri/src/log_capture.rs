//! Bounded managed output capture. Re-executed before Tauri initializes so capture
//! survives the UI's exit without changing the managed command's root PID.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

pub const MAX_BYTES: u64 = 5 * 1024 * 1024;
const MODE: &str = "--port-lens-log-writer";
const READY: &[u8] = b"PORT_LENS_LOG_READY\n";
const CHUNK_BYTES: usize = 8192;
const QUEUE_CHUNKS: usize = 64;

/// Called before the GUI/single-instance plugin. Never starts a Tauri window.
pub fn run_if_requested() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(MODE)) {
        return None;
    }
    let Some(path) = args.next() else {
        return Some(2);
    };
    let Some(header) = args.next() else {
        return Some(2);
    };
    if args.next().is_some() {
        return Some(2);
    }
    Some(run(Path::new(&path), &header.to_string_lossy()))
}

pub struct Capture {
    input: Option<File>,
    child: Option<Child>,
}

impl Capture {
    pub fn start(path: &Path, header: &str) -> io::Result<Self> {
        let mut command = Command::new(std::env::current_exe()?);
        #[cfg(not(test))]
        command.arg(MODE).arg(path).arg(header);
        // Unit tests re-execute their own harness; integration tests exercise the
        // shipped main() entry too. Both use precisely the same capture loop.
        #[cfg(test)]
        command
            .args([
                "--exact",
                "log_capture::tests::collector_process",
                "--ignored",
                "--nocapture",
            ])
            .env("PORT_LENS_TEST_LOG_PATH", path)
            .env("PORT_LENS_TEST_LOG_HEADER", header);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut output = child.stdout.take().expect("piped stdout");
        let (tx, rx) = mpsc::sync_channel(1);
        thread::spawn(move || {
            // The test harness emits a short preamble. Read a bounded handshake
            // in both modes, not an unbounded line supplied by another process.
            let mut seen = Vec::new();
            let result = (|| {
                for _ in 0..4096 {
                    let mut byte = [0];
                    if let Err(error) = output.read_exact(&mut byte) {
                        return Err(io::Error::other(format!(
                            "Log writer startup failed: {} ({error})",
                            String::from_utf8_lossy(&seen)
                        )));
                    }
                    seen.push(byte[0]);
                    if seen.ends_with(READY) {
                        return Ok(());
                    }
                }
                Err(io::Error::other("Invalid log writer handshake"))
            })();
            let _ = tx.send(result);
        });
        let ready = rx
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Log writer did not become ready"))
            .and_then(|result| result);
        if let Err(error) = ready {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        let input = child.stdin.take().expect("piped stdin");
        #[cfg(unix)]
        let input = File::from(std::os::fd::OwnedFd::from(input));
        #[cfg(windows)]
        let input = File::from(std::os::windows::io::OwnedHandle::from(input));
        Ok(Self {
            input: Some(input),
            child: Some(child),
        })
    }

    pub fn stdio(&self) -> io::Result<Stdio> {
        Ok(Stdio::from(
            self.input.as_ref().expect("open capture").try_clone()?,
        ))
    }

    pub fn finish(mut self, footer: String) {
        // Do not wait for descendants that still own the output pipe, or block
        // lifecycle/refresh work on a pipe write. The collector owns disk I/O.
        thread::spawn(move || {
            if let Some(mut input) = self.input.take() {
                let _ = input.write_all(footer.as_bytes());
            }
            if let Some(mut child) = self.child.take() {
                let _ = child.wait();
            }
        });
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.input.take();
        if let Some(mut child) = self.child.take() {
            // EOF ends unused collectors after spawn failures. Never kill a
            // collector whose pipe is still inherited by a running service.
            thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}

struct RotatingLog {
    path: PathBuf,
    file: Option<File>,
    length: u64,
    limit: u64,
    header: Vec<u8>,
    _lock: File,
}

impl RotatingLog {
    fn open(path: &Path, header: &str, limit: u64) -> io::Result<Self> {
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path.with_extension("log.lock"))?;
        lock.try_lock().map_err(|error| match error {
            fs::TryLockError::WouldBlock => io::Error::from(io::ErrorKind::WouldBlock),
            fs::TryLockError::Error(error) => error,
        })?;
        // Migrate old unlimited logs, retaining their newest bytes. Never load
        // an entire legacy file into memory. Only inactive files are touched.
        trim_tail(path, limit)?;
        trim_tail(&path.with_extension("log.1"), limit)?;
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let length = file.metadata()?.len();
        let mut result = Self {
            path: path.to_owned(),
            file: Some(file),
            length,
            limit,
            header: header
                .as_bytes()
                .iter()
                .copied()
                .take((limit / 4).min(1024) as usize)
                .collect(),
            _lock: lock,
        };
        let header = result.header.clone();
        result.append(&header)?;
        Ok(result)
    }

    fn rotate(&mut self) -> io::Result<()> {
        // Close our handle before Windows rename. If a viewer prevents rotation,
        // leave both files in place and retry later; never append past the cap.
        self.file.take();
        let previous = self.path.with_extension("log.1");
        match fs::remove_file(&previous) {
            Ok(()) => (),
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
        fs::rename(&self.path, &previous)?;
        self.length = 0;
        self.file = Some(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?,
        );
        if let Some(file) = self.file.as_mut() {
            file.write_all(&self.header)?;
            self.length = self.header.len() as u64;
        }
        Ok(())
    }

    fn append(&mut self, mut bytes: &[u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            // Re-read after a partial/failed write; never trust a stale length.
            if self.file.is_none() {
                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.path)?;
                self.length = file.metadata()?.len();
                self.file = Some(file);
            }
            self.length = self.file.as_ref().unwrap().metadata()?.len();
            if self.length >= self.limit {
                self.rotate()?;
            }
            let count = bytes.len().min((self.limit - self.length) as usize);
            self.file.as_mut().unwrap().write_all(&bytes[..count])?;
            self.length += count as u64;
            bytes = &bytes[count..];
        }
        Ok(())
    }
}

fn trim_tail(path: &Path, limit: u64) -> io::Result<()> {
    let mut file = match OpenOptions::new().read(true).write(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let size = file.metadata()?.len();
    if size <= limit {
        return Ok(());
    }
    let start = size - limit;
    let mut offset = 0;
    let mut buffer = [0; CHUNK_BYTES];
    while offset < limit {
        let count = buffer.len().min((limit - offset) as usize);
        file.seek(SeekFrom::Start(start + offset))?;
        file.read_exact(&mut buffer[..count])?;
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(&buffer[..count])?;
        offset += count as u64;
    }
    file.set_len(limit)
}

fn drain_input(
    mut input: impl Read,
    tx: mpsc::SyncSender<Vec<u8>>,
    dropped: &AtomicU64,
) -> io::Result<()> {
    let mut bytes = [0; CHUNK_BYTES];
    loop {
        let count = match input.read(&mut bytes) {
            Ok(0) => return Ok(()),
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        match tx.try_send(bytes[..count].to_vec()) {
            Ok(()) => (),
            Err(mpsc::TrySendError::Full(chunk) | mpsc::TrySendError::Disconnected(chunk)) => {
                dropped.fetch_add(chunk.len() as u64, Ordering::Relaxed);
            }
        }
    }
}

fn status(path: &Path, message: &str) {
    // Fixed-size latest warning, visible through the existing App log folder.
    // Failure to write the warning must not prevent draining the process pipe.
    let _ = fs::write(
        path.with_extension("log.capture-status"),
        &message.as_bytes()[..message.len().min(2048)],
    );
}

fn run(path: &Path, header: &str) -> i32 {
    let deadline = Instant::now() + Duration::from_secs(2);
    let opened = loop {
        match RotatingLog::open(path, header, MAX_BYTES) {
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(20));
            }
            result => break result,
        }
    };
    let mut writer = match opened {
        Ok(writer) => writer,
        Err(error) => {
            let message = format!("Log capture could not start: {error}\n");
            let _ = io::stdout().write_all(message.as_bytes());
            if error.kind() != io::ErrorKind::WouldBlock {
                status(path, &message);
            }
            return 1;
        }
    };
    let _ = fs::remove_file(path.with_extension("log.capture-status"));
    if io::stdout()
        .write_all(READY)
        .and_then(|_| io::stdout().flush())
        .is_err()
    {
        return 1;
    }
    let dropped = Arc::new(AtomicU64::new(0));
    let reader_dropped = dropped.clone();
    let (tx, rx) = mpsc::sync_channel(QUEUE_CHUNKS);
    let reader = thread::spawn(move || drain_input(io::stdin().lock(), tx, &reader_dropped));
    let mut retry_at = Instant::now();
    let mut lost = 0u64;
    while let Ok(chunk) = rx.recv() {
        lost = lost.saturating_add(dropped.swap(0, Ordering::Relaxed));
        if Instant::now() < retry_at {
            lost = lost.saturating_add(chunk.len() as u64);
            continue;
        }
        let result = (|| {
            if lost > 0 {
                writer.append(
                    format!(
                        "\n=== Port Lens capture: {lost} bytes omitted (slow/failed storage) ===\n"
                    )
                    .as_bytes(),
                )?;
                status(path, &format!("Log capture omitted {lost} bytes because storage could not keep up. Capture resumed.\n"));
                lost = 0;
            }
            writer.append(&chunk)
        })();
        if let Err(error) = result {
            lost = lost.saturating_add(chunk.len() as u64);
            status(path, &format!("Log capture write/rotation failed: {error}. Output is drained and discarded until storage recovers.\n"));
            retry_at = Instant::now() + Duration::from_secs(1);
        }
    }
    if let Ok(Err(error)) = reader.join() {
        status(path, &format!("Log capture input failed: {error}\n"));
    }
    lost = lost.saturating_add(dropped.load(Ordering::Relaxed));
    if lost > 0 {
        let message = format!("\n=== Port Lens capture ended: at least {lost} bytes omitted ===\n");
        let _ = writer.append(message.as_bytes());
        status(path, &message);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    fn directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "port-lens-bounded-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
    #[test]
    #[ignore = "subprocess entry used by capture integration tests"]
    fn collector_process() {
        let path = std::env::var_os("PORT_LENS_TEST_LOG_PATH").expect("internal test path");
        std::process::exit(run(
            Path::new(&path),
            &std::env::var("PORT_LENS_TEST_LOG_HEADER").unwrap(),
        ));
    }
    #[test]
    #[ignore = "subprocess producer for parent-exit coverage"]
    fn delayed_output_process() {
        thread::sleep(Duration::from_millis(300));
        io::stdout().write_all(b"AFTER-OWNER-EXIT\n").unwrap();
        std::process::exit(0);
    }

    #[test]
    #[ignore = "subprocess launcher for parent-exit coverage"]
    fn detaching_parent() {
        let path = std::env::var_os("PORT_LENS_TEST_LOG_PATH").unwrap();
        let capture = Capture::start(Path::new(&path), "parent-exit run\n").unwrap();
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "log_capture::tests::delayed_output_process",
                "--ignored",
                "--nocapture",
            ])
            .stdin(Stdio::null())
            .stdout(capture.stdio().unwrap())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        std::process::exit(0);
    }

    #[test]
    fn capture_survives_owner_exit_and_releases_lock_after_producer_eof() {
        let dir = directory();
        let path = dir.join("stdout.log");
        let result = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "log_capture::tests::detaching_parent",
                "--ignored",
                "--nocapture",
            ])
            .env("PORT_LENS_TEST_LOG_PATH", &path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(result.success());
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let finished = fs::read_to_string(&path)
                .unwrap_or_default()
                .contains("AFTER-OWNER-EXIT")
                && OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(path.with_extension("log.lock"))
                    .map(|file| file.try_lock().is_ok())
                    .unwrap_or(false);
            if finished {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "capture did not survive parent exit / finish at EOF"
            );
            thread::sleep(Duration::from_millis(20));
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn capture_drains_after_rotation_failure_and_restart_recovers() {
        let dir = directory();
        let path = dir.join("stderr.log");
        let mut capture = Capture::start(&path, "").unwrap();
        // Simulate storage/rotation refusal after capture is already running.
        fs::create_dir(path.with_extension("log.1")).unwrap();
        for index in 1..=80 {
            capture
                .input
                .as_mut()
                .unwrap()
                .write_all(&[b'x'; 65536])
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while fs::metadata(&path).unwrap().len() < index * 65536 {
                assert!(Instant::now() < deadline);
                thread::sleep(Duration::from_millis(5));
            }
        }
        capture
            .input
            .as_mut()
            .unwrap()
            .write_all(b"rotation fails here")
            .unwrap();
        let warning = path.with_extension("log.capture-status");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !fs::read_to_string(&warning)
            .unwrap_or_default()
            .contains("failed")
        {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        }
        // A failed sink still consumes well beyond the bounded queue's capacity.
        capture
            .input
            .as_mut()
            .unwrap()
            .write_all(&vec![b'y'; 2 * 1024 * 1024])
            .unwrap();
        capture.input.take();
        let mut helper = capture.child.take().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while helper.try_wait().unwrap().is_none() {
            if Instant::now() > deadline {
                let _ = helper.kill();
                let _ = helper.wait();
                panic!("failed capture did not drain");
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(fs::metadata(&path).unwrap().len() <= MAX_BYTES);
        assert!(fs::metadata(&warning).unwrap().len() <= 2048);
        fs::remove_dir(path.with_extension("log.1")).unwrap();
        let mut restarted = Capture::start(&path, "NEW-RUN\n").unwrap();
        restarted
            .input
            .as_mut()
            .unwrap()
            .write_all(b"AFTER-FAILURE\n")
            .unwrap();
        restarted.input.take();
        assert!(restarted.child.take().unwrap().wait().unwrap().success());
        assert!(fs::read_to_string(&path).unwrap().contains("AFTER-FAILURE"));
        assert!(!warning.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn every_rotated_generation_keeps_run_context() {
        let dir = directory();
        let path = dir.join("stdout.log");
        let mut writer = RotatingLog::open(&path, "RUN-42\n", 100).unwrap();
        writer.append(&[b'x'; 351]).unwrap();
        drop(writer);
        for file in [&path, &path.with_extension("log.1")] {
            let bytes = fs::read(file).unwrap();
            assert!(bytes.starts_with(b"RUN-42\n"));
            assert!(bytes.len() <= 100);
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rotation_bounds_long_lines_and_keeps_newest_bytes() {
        let dir = directory();
        let path = dir.join("stdout.log");
        let mut writer = RotatingLog::open(&path, "", 100).unwrap();
        let bytes: Vec<u8> = (0..1053).map(|i| (i % 251) as u8).collect();
        writer.append(&bytes).unwrap();
        drop(writer);
        let previous = fs::read(path.with_extension("log.1")).unwrap();
        let current = fs::read(&path).unwrap();
        assert_eq!(previous.len(), 100);
        assert_eq!(current.len(), 53);
        assert_eq!([previous, current].concat(), bytes[900..]);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn legacy_files_are_trimmed_and_concurrent_writer_is_rejected() {
        let dir = directory();
        let path = dir.join("stdout.log");
        fs::write(&path, [b'a'; 500]).unwrap();
        fs::write(path.with_extension("log.1"), [b'b'; 500]).unwrap();
        let writer = RotatingLog::open(&path, "", 100).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().len(), 100);
        assert_eq!(
            fs::metadata(path.with_extension("log.1")).unwrap().len(),
            100
        );
        assert!(RotatingLog::open(&path, "", 100).is_err());
        drop(writer);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn blocked_rotation_never_grows_file_and_can_recover() {
        let dir = directory();
        let path = dir.join("stderr.log");
        let mut writer = RotatingLog::open(&path, "", 100).unwrap();
        writer.append(&[b'a'; 100]).unwrap();
        fs::create_dir(path.with_extension("log.1")).unwrap();
        assert!(writer.append(b"latest").is_err());
        assert_eq!(fs::metadata(&path).unwrap().len(), 100);
        fs::remove_dir(path.with_extension("log.1")).unwrap();
        writer.append(b"latest").unwrap();
        drop(writer);
        assert_eq!(fs::read(&path).unwrap(), b"latest");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn full_or_disconnected_queue_keeps_draining_with_bounded_memory() {
        let (tx, rx) = mpsc::sync_channel(1);
        let lost = AtomicU64::new(0);
        drain_input(io::Cursor::new(vec![0; CHUNK_BYTES * 100]), tx, &lost).unwrap();
        assert_eq!(lost.load(Ordering::Relaxed), (CHUNK_BYTES * 99) as u64);
        assert_eq!(rx.recv().unwrap().len(), CHUNK_BYTES);
        let (tx, rx) = mpsc::sync_channel(1);
        drop(rx);
        drain_input(io::Cursor::new(vec![0; CHUNK_BYTES]), tx, &lost).unwrap();
        assert_eq!(lost.load(Ordering::Relaxed), (CHUNK_BYTES * 100) as u64);
    }
}
