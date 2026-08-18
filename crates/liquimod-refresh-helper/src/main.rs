//! F10 刷新提权 helper：监听命名管道，收到 "1" 向系统注入一次 F10。
//! 客户端（主 app）断开管道即退出，随 app 生命周期。
//! 由主 app 以 ShellExecuteW runas 提权启动（无清单，无键盘监听，无网络）。

use std::io::Read;

const PIPE: &str = r"\\.\pipe\liquimod-refresh";

/// 从字节流读数据，每批含 b'1' 即触发一次 on_poke。EOF/错误时返回（=退出）。
fn serve(mut read: impl Read, mut on_poke: impl FnMut()) {
    let mut buf = [0u8; 64];
    loop {
        match read.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                if buf[..n].contains(&b'1') {
                    on_poke();
                }
            }
        }
    }
}

#[cfg(windows)]
fn send_f10() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VK_F10,
    };
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
    unsafe { SendInput(&[down, up], std::mem::size_of::<INPUT>() as i32) };
}

#[cfg(not(windows))]
fn send_f10() {}

#[cfg(windows)]
fn main() {
    use std::os::windows::io::FromRawHandle;
    use windows::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows::Win32::Storage::FileSystem::PIPE_ACCESS_INBOUND;
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_TYPE_BYTE, PIPE_WAIT,
    };
    let wide: Vec<u16> = PIPE.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let handle = CreateNamedPipeW(
            windows::core::PCWSTR(wide.as_ptr()),
            PIPE_ACCESS_INBOUND,
            PIPE_TYPE_BYTE | PIPE_WAIT,
            1, // 单实例
            0,
            0,
            0,
            None,
        );
        if handle == INVALID_HANDLE_VALUE {
            return; // 已在运行或创建失败：直接退出
        }
        if ConnectNamedPipe(handle, None).is_err() {
            return;
        }
        let file = std::fs::File::from_raw_handle(handle.0);
        serve(file, send_f10);
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
        let data = b"111xx1"; // 两批读：第一模拟一次 read 返回 "111"，第二批 "xx1"？
                              // Cursor 一次 read 尽量填满缓冲——64 > 6，故只 read 一次，应触发 1 次。
        let mut count = 0;
        serve(std::io::Cursor::new(data.to_vec()), || count += 1);
        assert_eq!(count, 1);
    }

    #[test]
    fn batch_without_one_does_not_trigger() {
        let mut count = 0;
        serve(std::io::Cursor::new(b"hello".to_vec()), || count += 1);
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
        serve(chunks, || count += 1);
        assert_eq!(count, 2);
    }
}
