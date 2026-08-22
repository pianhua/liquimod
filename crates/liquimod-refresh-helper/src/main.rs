//! F10 刷新提权 helper：监听命名管道，收到进程名后向对应窗口注入一次 F10。
//! 客户端（主 app）断开管道即退出，随 app 生命周期。
//! 由主 app 以 ShellExecuteW runas 提权启动（无清单，无键盘监听，无网络）。

#![windows_subsystem = "windows"]

use std::io::{Read, Write};

#[cfg(windows)]
use std::sync::{Mutex, OnceLock};
#[cfg(windows)]
use std::time::{SystemTime, UNIX_EPOCH};

const PIPE: &str = r"\\.\pipe\liquimod-refresh";

#[cfg(windows)]
fn log_event(message: impl AsRef<str>) {
    static LOG_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _lock = LOG_LOCK.get_or_init(|| Mutex::new(())).lock().ok();
    let Some(exe) = std::env::current_exe().ok() else {
        return;
    };
    let Some(root) = exe.parent() else {
        return;
    };
    let log_dir = root.join("Logs");
    if std::fs::create_dir_all(&log_dir).is_err() {
        return;
    }
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("refresh-helper.log"))
    else {
        return;
    };
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default();
    let _ = writeln!(file, "[{timestamp}] {}", message.as_ref());
}

/// 从字节流读取 `p<process-name>\0` 命令并回写是否实际发送成功。
fn serve(mut read: impl Read, mut write: impl Write, mut on_poke: impl FnMut(&str) -> bool) {
    let mut buf = [0u8; 64];
    let mut pending = Vec::new();
    loop {
        match read.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                pending.extend_from_slice(&buf[..n]);
                loop {
                    match pending.first().copied() {
                        Some(b'1') => {
                            pending.drain(..1);
                            let ack = if on_poke("StarRail.exe") { b'1' } else { b'0' };
                            if write.write_all(&[ack]).is_err() || write.flush().is_err() {
                                return;
                            }
                        }
                        Some(b'p') => {
                            let Some(end) = pending.iter().position(|byte| *byte == 0) else {
                                break;
                            };
                            let command: Vec<u8> = pending.drain(..=end).collect();
                            let process_name = command
                                .strip_prefix(b"p")
                                .and_then(|value| value.strip_suffix(&[0]))
                                .and_then(|value| std::str::from_utf8(value).ok())
                                .filter(|value| !value.trim().is_empty())
                                .unwrap_or("StarRail.exe");
                            let ack = if on_poke(process_name) { b'1' } else { b'0' };
                            if write.write_all(&[ack]).is_err() || write.flush().is_err() {
                                return;
                            }
                        }
                        Some(_) => {
                            // 兼容旧客户端可能残留的无效字节，但不要把进程名中的数字 1
                            // 误判为旧协议命令。
                            let next_command = pending
                                .iter()
                                .position(|byte| *byte == b'1' || *byte == b'p');
                            match next_command {
                                Some(index) => {
                                    pending.drain(..index);
                                }
                                None => pending.clear(),
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    }
}

#[cfg(windows)]
fn send_f10(process_name: &str) -> bool {
    use sysinfo::{ProcessesToUpdate, System};
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE,
        VIRTUAL_KEY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowThreadProcessId,
        IsWindowVisible, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };

    struct WindowSearch {
        pid: u32,
        hwnd: Option<HWND>,
    }

    unsafe extern "system" fn find_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = unsafe { &mut *(lparam.0 as *mut WindowSearch) };
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid == search.pid && unsafe { IsWindowVisible(hwnd).as_bool() } {
            search.hwnd = Some(hwnd);
            return BOOL(0);
        }
        BOOL(1)
    }

    log_event(format!("F10 request process={process_name}"));
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let pids: Vec<u32> = system
        .processes()
        .iter()
        .filter(|(_, process)| process.name().eq_ignore_ascii_case(process_name))
        .map(|(pid, _)| pid.as_u32())
        .collect();
    log_event(format!("matched pids={pids:?}"));
    let mut hwnd = None;
    for pid in pids {
        let mut search = WindowSearch { pid, hwnd: None };
        let _ = unsafe { EnumWindows(Some(find_window), LPARAM(&mut search as *mut _ as isize)) };
        if search.hwnd.is_some() {
            hwnd = search.hwnd;
            break;
        }
    }
    let Some(hwnd) = hwnd else {
        log_event("no visible game window found");
        return false;
    };
    let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
    let mut focused = false;
    for _ in 0..5 {
        let _ = unsafe { BringWindowToTop(hwnd) };
        focused = unsafe { SetForegroundWindow(hwnd).as_bool() }
            || unsafe { GetForegroundWindow() } == hwnd;
        if focused {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(80));
    }
    if !focused {
        log_event(format!("failed to focus game window hwnd={hwnd:?}"));
        return false;
    }
    log_event(format!("focused game window hwnd={hwnd:?}"));
    std::thread::sleep(std::time::Duration::from_millis(180));

    // F10 的物理扫描码为 0x44。部分 Unity/DirectInput/3Dmigoto
    // 路径不会把仅带 wVk 的注入事件当成真实键盘输入。
    let down = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: 0x44,
                dwFlags: KEYEVENTF_SCANCODE,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let mut up = down;
    up.Anonymous.ki.dwFlags = KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP;
    let sent_down = unsafe { SendInput(&[down], std::mem::size_of::<INPUT>() as i32) };
    std::thread::sleep(std::time::Duration::from_millis(40));
    let sent_up = unsafe { SendInput(&[up], std::mem::size_of::<INPUT>() as i32) };
    let success = sent_down == 1 && sent_up == 1;
    log_event(format!(
        "SendInput scan_code=0x44 down={sent_down} up={sent_up} success={success}"
    ));
    success
}

#[cfg(not(windows))]
fn send_f10() -> bool {
    false
}

#[cfg(windows)]
fn main() {
    use std::os::windows::io::FromRawHandle;
    use windows::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_TYPE_BYTE, PIPE_WAIT,
    };
    log_event("refresh helper starting");
    let wide: Vec<u16> = PIPE.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let handle = CreateNamedPipeW(
            windows::core::PCWSTR(wide.as_ptr()),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_WAIT,
            1, // 单实例
            0,
            0,
            0,
            None,
        );
        if handle == INVALID_HANDLE_VALUE {
            log_event("CreateNamedPipeW failed");
            std::process::exit(2); // 已在运行或创建失败
        }
        if let Err(e) = ConnectNamedPipe(handle, None) {
            if e.code() != windows::Win32::Foundation::ERROR_PIPE_CONNECTED.into() {
                log_event(format!("ConnectNamedPipe failed code={:?}", e.code()));
                std::process::exit(3); // 连接失败
            }
        }
        log_event("refresh pipe connected");
        let file = std::fs::File::from_raw_handle(handle.0);
        let write = file.try_clone().unwrap_or_else(|_| std::process::exit(4));
        serve(file, write, send_f10);
        log_event("refresh pipe disconnected");
        // file drop → 句柄关闭 → 进程退出
    }
}

#[cfg(not(windows))]
fn main() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_batch_with_one_triggers_once_and_eof_stops() {
        let mut count = 0;
        let mut output = Vec::new();
        serve(
            std::io::Cursor::new(b"pStarRail.exe\0".to_vec()),
            &mut output,
            |_| {
                count += 1;
                true
            },
        );
        assert_eq!(count, 1);
        assert_eq!(output, b"1");
    }

    #[test]
    fn batch_without_one_does_not_trigger() {
        let mut count = 0;
        serve(std::io::Cursor::new(b"hello".to_vec()), Vec::new(), |_| {
            count += 1;
            true
        });
        assert_eq!(count, 0);
    }

    #[test]
    fn split_batches_each_trigger() {
        // 模拟分两次到达：用按块迭代的 reader
        struct Chunked(Vec<std::io::Cursor<Vec<u8>>>);
        impl std::io::Read for Chunked {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.0.is_empty() {
                    return Ok(0);
                }
                let n = self.0[0].read(buf)?;
                if n == 0 {
                    self.0.remove(0);
                    return self.read(buf);
                }
                Ok(n)
            }
        }
        let chunks = Chunked(vec![
            std::io::Cursor::new(b"pStarRail".to_vec()),
            std::io::Cursor::new(b"zz".to_vec()),
            std::io::Cursor::new(b".exe\0".to_vec()),
            std::io::Cursor::new(b"pStarRail.exe\0".to_vec()),
        ]);
        let mut count = 0;
        let mut output = Vec::new();
        serve(chunks, &mut output, |_| {
            count += 1;
            true
        });
        assert_eq!(count, 2);
        assert_eq!(output, b"11");
    }

    #[test]
    fn failed_poke_writes_negative_ack() {
        let mut output = Vec::new();
        serve(
            std::io::Cursor::new(b"pStarRail.exe\0".to_vec()),
            &mut output,
            |_| false,
        );
        assert_eq!(output, b"0");
    }

    #[test]
    fn process_names_containing_one_are_not_legacy_commands() {
        let mut output = Vec::new();
        let mut received = String::new();
        serve(
            std::io::Cursor::new(b"pGame1.exe\0".to_vec()),
            &mut output,
            |process| {
                received = process.to_string();
                true
            },
        );
        assert_eq!(received, "Game1.exe");
        assert_eq!(output, b"1");
    }
}
