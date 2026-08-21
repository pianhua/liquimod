//! F10 刷新提权 helper：监听命名管道，收到 "1" 向系统注入一次 F10。
//! 客户端（主 app）断开管道即退出，随 app 生命周期。
//! 由主 app 以 ShellExecuteW runas 提权启动（无清单，无键盘监听，无网络）。

#![windows_subsystem = "windows"]

use std::io::{Read, Write};

const PIPE: &str = r"\\.\pipe\liquimod-refresh";

/// 从字节流读数据，每批含 b'1' 即触发一次 on_poke，并回写是否实际发送成功。
fn serve(mut read: impl Read, mut write: impl Write, mut on_poke: impl FnMut() -> bool) {
    let mut buf = [0u8; 64];
    loop {
        match read.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                if buf[..n].contains(&b'1') {
                    let ack = if on_poke() { b'1' } else { b'0' };
                    if write.write_all(&[ack]).is_err() || write.flush().is_err() {
                        return;
                    }
                }
            }
        }
    }
}

#[cfg(windows)]
fn send_f10() -> bool {
    use sysinfo::{ProcessesToUpdate, System};
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VK_F10,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow, ShowWindow,
        SW_RESTORE,
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

    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let Some(pid) = system.processes().iter().find_map(|(pid, process)| {
        process
            .name()
            .eq_ignore_ascii_case("StarRail.exe")
            .then(|| pid.as_u32())
    }) else {
        return false;
    };
    let mut search = WindowSearch { pid, hwnd: None };
    let _ = unsafe { EnumWindows(Some(find_window), LPARAM(&mut search as *mut _ as isize)) };
    let Some(hwnd) = search.hwnd else {
        return false;
    };
    let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
    if !unsafe { SetForegroundWindow(hwnd).as_bool() } {
        return false;
    }
    std::thread::sleep(std::time::Duration::from_millis(120));

    let down = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_F10,
                wScan: 0,
                dwFlags: KEYBD_EVENT_FLAGS(0),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let mut up = down;
    up.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
    (unsafe { SendInput(&[down, up], std::mem::size_of::<INPUT>() as i32) }) == 2
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
            std::process::exit(2); // 已在运行或创建失败
        }
        if let Err(e) = ConnectNamedPipe(handle, None) {
            if e.code() != windows::Win32::Foundation::ERROR_PIPE_CONNECTED.into() {
                std::process::exit(3); // 连接失败
            }
        }
        let file = std::fs::File::from_raw_handle(handle.0);
        let write = file.try_clone().unwrap_or_else(|_| std::process::exit(4));
        serve(file, write, send_f10);
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
        let data = b"111xx1"; // Cursor 一次 read 返回全部 6 字节（64 > 6），含 b'1' → 触发 1 次
        let mut count = 0;
        let mut output = Vec::new();
        serve(std::io::Cursor::new(data.to_vec()), &mut output, || {
            count += 1;
            true
        });
        assert_eq!(count, 1);
        assert_eq!(output, b"1");
    }

    #[test]
    fn batch_without_one_does_not_trigger() {
        let mut count = 0;
        serve(std::io::Cursor::new(b"hello".to_vec()), Vec::new(), || {
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
            std::io::Cursor::new(b"1".to_vec()),
            std::io::Cursor::new(b"zz".to_vec()),
            std::io::Cursor::new(b"1".to_vec()),
        ]);
        let mut count = 0;
        let mut output = Vec::new();
        serve(chunks, &mut output, || {
            count += 1;
            true
        });
        assert_eq!(count, 2);
        assert_eq!(output, b"11");
    }

    #[test]
    fn failed_poke_writes_negative_ack() {
        let mut output = Vec::new();
        serve(std::io::Cursor::new(b"1".to_vec()), &mut output, || false);
        assert_eq!(output, b"0");
    }
}
