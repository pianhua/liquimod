//! 3DMigoto 游戏原生伴侣与 F10 刷新提权 Helper。
//!
//! 监听命名管道，支持两类操作：
//! 1. 收到 "1"：前置游戏窗口并发送 F10 热重载按键；
//! 2. 收到 "LAUNCH|..."：带 3DMigoto Hook (3dmloader.dll / Win32 Hook) 一键拉起游戏。
//!
//! 由主 app 随生命周期管理或按需提权唤起。

#![windows_subsystem = "windows"]

use std::io::{Read, Write};

const PIPE: &str = r"\\.\pipe\liquimod-refresh";

/// 从字节流读数据，支持 F10 刷新 (b'1') 与 一键启动注入 (LAUNCH|...)
fn serve(
    mut read: impl Read,
    mut write: impl Write,
    mut on_poke: impl FnMut() -> bool,
    mut on_launch: impl FnMut(&str, &str, &str, &str) -> bool,
) {
    let mut buf = [0u8; 1024];
    loop {
        match read.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                let text = String::from_utf8_lossy(&buf[..n]);
                if text.starts_with("LAUNCH|") {
                    let parts: Vec<&str> = text.trim().split('|').collect();
                    let game_exe = parts.get(1).copied().unwrap_or("");
                    let work_dir = parts.get(2).copied().unwrap_or("");
                    let d3d11_dll = parts.get(3).copied().unwrap_or("");
                    let loader_dll = parts.get(4).copied().unwrap_or("");
                    let ok = on_launch(game_exe, work_dir, d3d11_dll, loader_dll);
                    let ack = if ok { b"L1" } else { b"L0" };
                    if write.write_all(ack).is_err() || write.flush().is_err() {
                        return;
                    }
                } else if buf[..n].contains(&b'1') {
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

#[cfg(windows)]
fn launch_and_inject(game_exe: &str, work_dir: &str, d3d11_dll: &str, loader_dll: &str) -> bool {
    use std::path::Path;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{FreeLibrary, HANDLE};
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows::Win32::System::Threading::{CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW};
    use windows::Win32::UI::WindowsAndMessaging::HHOOK;

    let game_path = Path::new(game_exe);
    if !game_path.is_file() {
        return false;
    }
    let work_path = if work_dir.is_empty() {
        game_path.parent().unwrap_or(Path::new("."))
    } else {
        Path::new(work_dir)
    };

    let d3d11_path = Path::new(d3d11_dll);
    let loader_path = Path::new(loader_dll);

    // 1. 如果存在 3dmloader.dll，优先使用 XXMI 标准 Windows Hook 注入
    if loader_path.is_file() && d3d11_path.is_file() {
        let loader_wide: Vec<u16> = loader_path
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let h_loader = unsafe { LoadLibraryW(PCWSTR(loader_wide.as_ptr())) };
        if let Ok(h_loader) = h_loader {
            type FnHookLibrary = unsafe extern "system" fn(PCWSTR, *mut HHOOK, *mut HANDLE) -> i32;
            type FnWaitForInjection = unsafe extern "system" fn(PCWSTR, PCWSTR, i32) -> i32;
            type FnUnhookLibrary = unsafe extern "system" fn(*mut HHOOK, *mut HANDLE) -> i32;
            type FnStartProcess = unsafe extern "system" fn(PCWSTR, PCWSTR, PCWSTR) -> i32;

            let p_hook = unsafe { GetProcAddress(h_loader, windows::core::s!("HookLibrary")) };
            let p_wait = unsafe { GetProcAddress(h_loader, windows::core::s!("WaitForInjection")) };
            let p_unhook = unsafe { GetProcAddress(h_loader, windows::core::s!("UnhookLibrary")) };
            let p_start = unsafe { GetProcAddress(h_loader, windows::core::s!("StartProcess")) };

            if let (Some(f_hook), Some(f_wait), Some(f_unhook)) = (p_hook, p_wait, p_unhook) {
                let fn_hook: FnHookLibrary = unsafe { std::mem::transmute(f_hook) };
                let fn_wait: FnWaitForInjection = unsafe { std::mem::transmute(f_wait) };
                let fn_unhook: FnUnhookLibrary = unsafe { std::mem::transmute(f_unhook) };

                let d3d11_wide: Vec<u16> = d3d11_path
                    .to_string_lossy()
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                let mut hook = HHOOK::default();
                let mut mutex = HANDLE::default();

                let hook_res =
                    unsafe { fn_hook(PCWSTR(d3d11_wide.as_ptr()), &mut hook, &mut mutex) };
                if hook_res == 0 {
                    // Hook 成功，启动游戏进程
                    let si = STARTUPINFOW {
                        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
                        ..Default::default()
                    };
                    let mut pi = PROCESS_INFORMATION::default();

                    let app_name_wide: Vec<u16> = game_path
                        .to_string_lossy()
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();
                    let work_dir_wide: Vec<u16> = work_path
                        .to_string_lossy()
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();

                    let started = unsafe {
                        CreateProcessW(
                            PCWSTR(app_name_wide.as_ptr()),
                            None,
                            None,
                            None,
                            false,
                            windows::Win32::System::Threading::PROCESS_CREATION_FLAGS(0),
                            None,
                            PCWSTR(work_dir_wide.as_ptr()),
                            &si,
                            &mut pi,
                        )
                    };

                    let launched = if started.is_ok() {
                        true
                    } else if let Some(f_start) = p_start {
                        let fn_start: FnStartProcess = unsafe { std::mem::transmute(f_start) };
                        let empty_wide: Vec<u16> = vec![0];
                        let res = unsafe {
                            fn_start(
                                PCWSTR(app_name_wide.as_ptr()),
                                PCWSTR(work_dir_wide.as_ptr()),
                                PCWSTR(empty_wide.as_ptr()),
                            )
                        };
                        res == 0
                    } else {
                        false
                    };

                    if launched {
                        let proc_name = game_path.file_name().unwrap_or_default().to_string_lossy();
                        let proc_wide: Vec<u16> =
                            proc_name.encode_utf16().chain(std::iter::once(0)).collect();
                        let _ = unsafe {
                            fn_wait(PCWSTR(d3d11_wide.as_ptr()), PCWSTR(proc_wide.as_ptr()), 15)
                        };
                        let _ = unsafe { fn_unhook(&mut hook, &mut mutex) };
                        let _ = unsafe { FreeLibrary(h_loader) };
                        return true;
                    }
                    let _ = unsafe { fn_unhook(&mut hook, &mut mutex) };
                }
            }
            let _ = unsafe { FreeLibrary(h_loader) };
        }
    }

    // 2. 备用原生启动模式：直接启动游戏进程
    let si = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut pi = PROCESS_INFORMATION::default();

    let app_name_wide: Vec<u16> = game_path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let work_dir_wide: Vec<u16> = work_path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let started = unsafe {
        CreateProcessW(
            PCWSTR(app_name_wide.as_ptr()),
            None,
            None,
            None,
            false,
            windows::Win32::System::Threading::PROCESS_CREATION_FLAGS(0),
            None,
            PCWSTR(work_dir_wide.as_ptr()),
            &si,
            &mut pi,
        )
    };

    started.is_ok()
}

#[cfg(not(windows))]
fn send_f10() -> bool {
    false
}

#[cfg(not(windows))]
fn launch_and_inject(
    _game_exe: &str,
    _work_dir: &str,
    _d3d11_dll: &str,
    _loader_dll: &str,
) -> bool {
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
        serve(file, write, send_f10, launch_and_inject);
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
        let data = b"111xx1";
        let mut count = 0;
        let mut output = Vec::new();
        serve(
            std::io::Cursor::new(data.to_vec()),
            &mut output,
            || {
                count += 1;
                true
            },
            |_, _, _, _| false,
        );
        assert_eq!(count, 1);
        assert_eq!(output, b"1");
    }

    #[test]
    fn launch_command_triggers_on_launch() {
        let data = b"LAUNCH|C:\\game.exe|C:\\game|C:\\d3d11.dll|C:\\3dmloader.dll";
        let mut launched = false;
        let mut output = Vec::new();
        serve(
            std::io::Cursor::new(data.to_vec()),
            &mut output,
            || false,
            |exe, dir, d3d, loader| {
                assert_eq!(exe, "C:\\game.exe");
                assert_eq!(dir, "C:\\game");
                assert_eq!(d3d, "C:\\d3d11.dll");
                assert_eq!(loader, "C:\\3dmloader.dll");
                launched = true;
                true
            },
        );
        assert!(launched);
        assert_eq!(output, b"L1");
    }

    #[test]
    fn batch_without_one_does_not_trigger() {
        let mut count = 0;
        serve(
            std::io::Cursor::new(b"hello".to_vec()),
            Vec::new(),
            || {
                count += 1;
                true
            },
            |_, _, _, _| false,
        );
        assert_eq!(count, 0);
    }

    #[test]
    fn failed_poke_writes_negative_ack() {
        let mut output = Vec::new();
        serve(
            std::io::Cursor::new(b"1".to_vec()),
            &mut output,
            || false,
            |_, _, _, _| false,
        );
        assert_eq!(output, b"0");
    }
}
