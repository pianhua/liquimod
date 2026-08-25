#![cfg(windows)]

use liquimod_core::refresh::{current_user_sid, PIPE_NAME};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const PROCESS_TIMEOUT: Duration = Duration::from_secs(5);
const PIPE_POLL_INTERVAL: Duration = Duration::from_millis(25);

struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    fn spawn(args: &[String]) -> io::Result<Self> {
        let executable = std::env::var_os("CARGO_BIN_EXE_liquimod-refresh-helper")
            .map(PathBuf::from)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "CARGO_BIN_EXE_liquimod-refresh-helper is not set",
                )
            })?;
        let child = Command::new(executable)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(Self { child: Some(child) })
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> io::Result<ExitStatus> {
        let deadline = Instant::now() + timeout;
        let child = self
            .child
            .as_mut()
            .ok_or_else(|| io::Error::other("helper child already consumed"))?;
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "refresh helper did not exit in time",
                ));
            }
            std::thread::sleep(PIPE_POLL_INTERVAL);
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let Some(child) = self.child.as_mut() else {
            return;
        };
        let running = child
            .try_wait()
            .map(|status| status.is_none())
            .unwrap_or(true);
        if running {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn connect_to_spawned_pipe(child: &mut ChildGuard) -> io::Result<File> {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        match OpenOptions::new().read(true).write(true).open(PIPE_NAME) {
            Ok(pipe) => return Ok(pipe),
            Err(error) => {
                if let Some(status) = child
                    .child
                    .as_mut()
                    .and_then(|value| value.try_wait().ok().flatten())
                {
                    return Err(io::Error::other(format!(
                        "refresh helper exited before pipe connection: {status}"
                    )));
                }
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("refresh pipe did not become available: {error}"),
                    ));
                }
                std::thread::sleep(PIPE_POLL_INTERVAL);
            }
        }
    }
}

fn wait_for_pipe_bytes(pipe: &File, timeout: Duration) -> io::Result<()> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Pipes::PeekNamedPipe;

    let deadline = Instant::now() + timeout;
    let handle = HANDLE(pipe.as_raw_handle());
    loop {
        let mut available = 0u32;
        unsafe { PeekNamedPipe(handle, None, 0, None, Some(&mut available), None) }
            .map_err(io::Error::other)?;
        if available > 0 {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "refresh helper did not reply in time",
            ));
        }
        std::thread::sleep(PIPE_POLL_INTERVAL);
    }
}

fn read_line(pipe: &mut File) -> io::Result<Vec<u8>> {
    let mut line = Vec::new();
    loop {
        wait_for_pipe_bytes(pipe, PROCESS_TIMEOUT)?;
        let mut byte = [0u8; 1];
        pipe.read_exact(&mut byte)?;
        line.push(byte[0]);
        if byte[0] == b'\n' {
            return Ok(line);
        }
        if line.len() > 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "refresh helper reply exceeded test limit",
            ));
        }
    }
}

fn make_paths() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let temp = tempfile::tempdir().expect("create integration test temp directory");
    let root = temp.path().join("LiquiMod helper integration with spaces");
    let data_root = root.join("Data Root with spaces");
    let game = root.join("Pinned Game with spaces.exe");
    let other_game = root.join("Other Game with spaces.exe");
    std::fs::create_dir_all(&data_root).expect("create pinned data root");
    std::fs::write(&game, b"not a real executable").expect("create pinned game fixture");
    std::fs::write(&other_game, b"not a real executable").expect("create mismatch fixture");
    (temp, game, other_game, data_root)
}

fn valid_args(sid: &str, game: &Path, data_root: &Path) -> Vec<String> {
    vec![
        format!("--user-sid={sid}"),
        format!("--game-exe={}", game.display()),
        format!("--data-root={}", data_root.display()),
    ]
}

fn assert_rejected_args(args: Vec<String>) {
    let mut child = ChildGuard::spawn(&args).expect("spawn helper for argument validation");
    let status = child
        .wait_for_exit(PROCESS_TIMEOUT)
        .expect("invalid helper arguments must terminate promptly");
    assert_eq!(
        status.code(),
        Some(2),
        "unexpected helper exit status: {status}"
    );
}

#[test]
fn real_helper_validates_pins_and_accepts_same_user_pipe_with_spaces() {
    let sid = current_user_sid().expect("integration test requires a domain user SID");
    let (_temp, game, other_game, data_root) = make_paths();

    // The helper uses a fixed production pipe and FILE_FLAG_FIRST_PIPE_INSTANCE;
    // keep this process-level test single-instance and ensure every failure path
    // reaps the child through ChildGuard.
    let args = valid_args(&sid, &game, &data_root);
    let mut child = ChildGuard::spawn(&args).expect("spawn real refresh helper");
    {
        let mut pipe = connect_to_spawned_pipe(&mut child).expect("connect to real helper pipe");

        // The test process and helper have the same SID. A nonexistent window
        // produces a negative F10 acknowledgement while proving authenticated IPC.
        pipe.write_all(b"pLiquiModNoVisibleWindow.exe\0")
            .expect("write F10 request");
        pipe.flush().expect("flush F10 request");
        let mut ack = [0u8; 1];
        wait_for_pipe_bytes(&pipe, PROCESS_TIMEOUT).expect("wait for F10 acknowledgement");
        pipe.read_exact(&mut ack).expect("read F10 acknowledgement");
        assert_eq!(ack, [b'0']);

        // The alternate path is a valid .exe fixture with spaces, but does not
        // match the canonical game path pinned in helper argv.
        let request = format!("LAUNCH|{}\n", other_game.display());
        pipe.write_all(request.as_bytes())
            .expect("write mismatched launch request");
        pipe.flush().expect("flush mismatched launch request");
        assert_eq!(
            read_line(&mut pipe).expect("read pin rejection"),
            b"L0|pinned\n"
        );
    }

    child
        .wait_for_exit(PROCESS_TIMEOUT)
        .expect("helper must exit after authenticated client disconnects");
}

#[test]
fn real_helper_rejects_missing_duplicate_unknown_and_unpaired_pin_args() {
    let sid = current_user_sid().expect("integration test requires a domain user SID");
    let (_temp, game, _other_game, data_root) = make_paths();

    assert_rejected_args(Vec::new());
    assert_rejected_args(vec![
        format!("--user-sid={sid}"),
        format!("--user-sid={sid}"),
    ]);
    assert_rejected_args(vec![
        format!("--user-sid={sid}"),
        "--unknown=value".to_string(),
    ]);
    assert_rejected_args(vec![
        format!("--user-sid={sid}"),
        format!("--game-exe={}", game.display()),
    ]);
    assert_rejected_args(vec![
        format!("--user-sid={sid}"),
        format!("--data-root={}", data_root.display()),
    ]);
}
