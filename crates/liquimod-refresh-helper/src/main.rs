//! 提权刷新/启动 helper：监听 v6 命名管道，负责 F10 与 3DMigoto 注入启动。
//! 主 app 以普通权限运行；helper 仅接受启动时钉扎的用户 SID、游戏路径与数据根目录。

#![windows_subsystem = "windows"]

use liquimod_core::launcher::{GameLaunchOptions, LaunchResult};
use liquimod_core::migoto_sync::runtime_paths;
use liquimod_core::refresh::is_valid_user_sid_text;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const PIPE: &str = r"\\.\pipe\liquimod-refresh-v6";
const MAX_COMMAND_BYTES: usize = 32 * 1024;
const USER_SID_ARG_PREFIX: &str = "--user-sid=";
const GAME_EXE_ARG_PREFIX: &str = "--game-exe=";
const DATA_ROOT_ARG_PREFIX: &str = "--data-root=";

#[derive(Debug, Clone, PartialEq, Eq)]
struct LaunchPin {
    user_sid: String,
    game_exe: Option<PathBuf>,
    data_root: Option<PathBuf>,
}

/// 只允许由普通用户 SID 组成的 DACL 文本；SID 校验在 core 与 helper 两端均执行。
fn pipe_security_sddl_text(user_sid: &str) -> Result<String, String> {
    if !is_valid_user_sid_text(user_sid) {
        return Err("用户 SID 格式无效".to_string());
    }
    Ok(format!("D:P(A;;GA;;;{user_sid})"))
}

fn invalid_path(path: &Path) -> bool {
    let value = path.to_string_lossy();
    value.is_empty()
        || !path.is_absolute()
        || value
            .chars()
            .any(|character| matches!(character, '\0' | '"' | '\r' | '\n' | '|'))
}

fn validate_pin(pin: &LaunchPin) -> Result<(), String> {
    if !is_valid_user_sid_text(&pin.user_sid) {
        return Err("用户 SID 格式无效".to_string());
    }
    if let Some(game_exe) = &pin.game_exe {
        if invalid_path(game_exe)
            || !game_exe.is_file()
            || game_exe
                .extension()
                .and_then(|extension| extension.to_str())
                .is_none_or(|extension| !extension.eq_ignore_ascii_case("exe"))
        {
            return Err(format!("游戏路径无效: {}", game_exe.display()));
        }
    }
    if let Some(data_root) = &pin.data_root {
        if invalid_path(data_root) || !data_root.is_dir() {
            return Err(format!("数据根目录无效: {}", data_root.display()));
        }
    }
    if pin.game_exe.is_some() != pin.data_root.is_some() {
        return Err("游戏路径与数据根目录必须成对提供".to_string());
    }
    Ok(())
}

fn parse_pin_args<I>(args: I) -> Result<LaunchPin, String>
where
    I: IntoIterator<Item = String>,
{
    let mut user_sid = None;
    let mut game_exe = None;
    let mut data_root = None;
    for arg in args {
        if let Some(value) = arg.strip_prefix(USER_SID_ARG_PREFIX) {
            if user_sid.replace(value.to_string()).is_some() {
                return Err("重复的 --user-sid 参数".to_string());
            }
        } else if let Some(value) = arg.strip_prefix(GAME_EXE_ARG_PREFIX) {
            if game_exe.replace(PathBuf::from(value)).is_some() {
                return Err("重复的 --game-exe 参数".to_string());
            }
        } else if let Some(value) = arg.strip_prefix(DATA_ROOT_ARG_PREFIX) {
            if data_root.replace(PathBuf::from(value)).is_some() {
                return Err("重复的 --data-root 参数".to_string());
            }
        } else {
            return Err(format!("未知或未格式化参数: {arg}"));
        }
    }
    let pin = LaunchPin {
        user_sid: user_sid.ok_or_else(|| "缺少 --user-sid 参数".to_string())?,
        game_exe,
        data_root,
    };
    validate_pin(&pin)?;
    Ok(pin)
}

fn sanitize_frame_text(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

fn write_frame(mut write: impl Write, frame: &str) -> std::io::Result<()> {
    // WriteFile 已将数据交给管道缓冲区；不要调用 FlushFileBuffers。
    // 后者会等待对端读走数据，使恶意/断开的同用户客户端能够永久占住单实例管道。
    write.write_all(frame.as_bytes())
}

fn launch_reply_frame(result: &LaunchResult) -> String {
    if !result.success {
        return failed_reply_frame(&result.message);
    }
    let pid = result
        .pid
        .map(|value| value.to_string())
        .unwrap_or_default();
    format!("L1|{}|{}\n", pid, sanitize_frame_text(&result.message))
}

fn failed_reply_frame(reason: &str) -> String {
    format!("L0|{}\n", sanitize_frame_text(reason))
}

fn validate_launch_request(request: &str, pin: &LaunchPin) -> Result<PathBuf, &'static str> {
    if request.is_empty()
        || request
            .chars()
            .any(|character| matches!(character, '\0' | '|' | '\r' | '\n' | '"'))
    {
        return Err("pinned");
    }
    let Some(pinned) = pin.game_exe.as_deref() else {
        return Err("pinned");
    };
    let requested = PathBuf::from(request);
    if invalid_path(&requested) || !requested.is_file() {
        return Err("pinned");
    }
    let requested = requested.canonicalize().map_err(|_| "pinned")?;
    let pinned = pinned.canonicalize().map_err(|_| "pinned")?;
    if requested != pinned {
        return Err("pinned");
    }
    Ok(requested)
}

/// 处理 v6 字节流协议。`serve` 保留旧测试所用的三参数入口，生产使用带 LAUNCH 回调的版本。
#[cfg_attr(not(test), allow(dead_code))]
fn serve<R, W, P>(read: R, write: W, on_poke: P)
where
    R: Read,
    W: Write,
    P: FnMut(&str) -> bool,
{
    serve_with_launch(read, write, on_poke, |_request, _stage| {
        Err("launch unsupported".to_string())
    });
}

fn serve_with_launch<R, W, P, L>(mut read: R, mut write: W, mut on_poke: P, mut on_launch: L)
where
    R: Read,
    W: Write,
    P: FnMut(&str) -> bool,
    L: FnMut(&str, &mut dyn FnMut(&str)) -> Result<LaunchResult, String>,
{
    let mut buf = [0u8; 4096];
    let mut pending = Vec::new();
    loop {
        match read.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                pending.extend_from_slice(&buf[..n]);
                if pending.len() > MAX_COMMAND_BYTES {
                    tracing::warn!(
                        size = pending.len(),
                        "refresh helper command buffer exceeded limit"
                    );
                    return;
                }
                while let Some(first) = pending.first().copied() {
                    match first {
                        b'1' => {
                            pending.drain(..1);
                            let ack = if on_poke("StarRail.exe") { b'1' } else { b'0' };
                            if write.write_all(&[ack]).is_err() {
                                return;
                            }
                        }
                        b'p' => {
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
                            if write.write_all(&[ack]).is_err() {
                                return;
                            }
                        }
                        b'L' => {
                            let Some(end) = pending.iter().position(|byte| *byte == b'\n') else {
                                break;
                            };
                            let command: Vec<u8> = pending.drain(..=end).collect();
                            let Some(request) = command
                                .strip_suffix(b"\n")
                                .and_then(|value| std::str::from_utf8(value).ok())
                                .and_then(|value| value.strip_prefix("LAUNCH|"))
                            else {
                                let _ = write_frame(&mut write, "E|pinned\n");
                                continue;
                            };
                            let mut write_failed = false;
                            let mut send_stage = |stage: &str| {
                                tracing::info!(stage, "launch lifecycle stage");
                                let frame = format!("S{}\n", sanitize_frame_text(stage));
                                if write_frame(&mut write, &frame).is_err() {
                                    write_failed = true;
                                }
                            };
                            let result = on_launch(request, &mut send_stage);
                            if write_failed {
                                return;
                            }
                            let frame = match result {
                                Ok(result) => launch_reply_frame(&result),
                                Err(reason) => failed_reply_frame(&reason),
                            };
                            if write_frame(&mut write, &frame).is_err() {
                                return;
                            }
                        }
                        _ => {
                            // 丢弃噪声并重新同步到下一条完整命令起点；L 是 v6 新命令。
                            let next_command = pending
                                .iter()
                                .position(|byte| *byte == b'1' || *byte == b'p' || *byte == b'L');
                            match next_command {
                                Some(index) if index > 0 => {
                                    pending.drain(..index);
                                }
                                Some(0) => {
                                    // The first byte is noise; discard it and keep looking for
                                    // the next bounded command frame.
                                    pending.drain(..1);
                                }
                                Some(index) => {
                                    pending.drain(..index);
                                }
                                None => {
                                    pending.clear();
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(windows)]
fn log_event(message: impl AsRef<str>) {
    tracing::info!("{}", message.as_ref());
}

#[cfg(windows)]
fn init_tracing() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let exe = std::env::current_exe().ok()?;
    let root = exe.parent()?;
    let log_dir = root.join("Logs");
    std::fs::create_dir_all(&log_dir).ok()?;
    let appender = tracing_appender::rolling::never(log_dir, "refresh-helper.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .try_init()
        .ok()?;
    Some(guard)
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
fn send_f10(_process_name: &str) -> bool {
    false
}

#[cfg(windows)]
struct PipeSecurity {
    descriptor: windows::Win32::Security::PSECURITY_DESCRIPTOR,
    attributes: windows::Win32::Security::SECURITY_ATTRIBUTES,
}

#[cfg(windows)]
impl Drop for PipeSecurity {
    fn drop(&mut self) {
        use windows::Win32::Foundation::{LocalFree, HLOCAL};
        unsafe {
            let _ = LocalFree(Some(HLOCAL(self.descriptor.0.cast())));
        }
    }
}

#[cfg(windows)]
fn create_pipe_security(user_sid: &str) -> Result<PipeSecurity, String> {
    use windows::core::PCWSTR;
    use windows::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
    use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};

    let sddl = pipe_security_sddl_text(user_sid)?;
    let wide: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(wide.as_ptr()),
            1,
            &mut descriptor,
            None,
        )
        .map_err(|error| format!("创建管道安全描述符失败: {error}"))?;
    }
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };
    Ok(PipeSecurity {
        descriptor,
        attributes,
    })
}

#[cfg(windows)]
fn client_matches_user(
    pipe: windows::Win32::Foundation::HANDLE,
    expected_sid: &str,
) -> Result<bool, String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
    use windows::Win32::Security::Authorization::ConvertStringSidToSidW;
    use windows::Win32::Security::{
        EqualSid, GetTokenInformation, TokenUser, PSID, TOKEN_QUERY, TOKEN_USER,
    };
    use windows::Win32::System::Pipes::GetNamedPipeClientProcessId;
    use windows::Win32::System::Threading::{
        OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let mut pid = 0u32;
    unsafe { GetNamedPipeClientProcessId(pipe, &mut pid) }
        .map_err(|error| format!("获取管道客户端进程 ID 失败: {error}"))?;
    if pid == 0 {
        return Err("管道客户端进程 ID 为空".to_string());
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
        .map_err(|error| format!("打开管道客户端进程失败: {error}"))?;
    let mut token = HANDLE::default();
    let token_result = unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) };
    unsafe {
        let _ = CloseHandle(process);
    }
    token_result.map_err(|error| format!("读取管道客户端令牌失败: {error}"))?;

    let result = (|| {
        let mut required = 0u32;
        let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut required) };
        if required == 0 {
            return Err("获取管道客户端 SID 缓冲区大小失败".to_string());
        }
        let units = (required as usize).div_ceil(std::mem::size_of::<usize>());
        let mut buffer = vec![0usize; units];
        unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                Some(buffer.as_mut_ptr().cast()),
                (buffer.len() * std::mem::size_of::<usize>()) as u32,
                &mut required,
            )
            .map_err(|error| format!("读取管道客户端 SID 失败: {error}"))?;
        }
        let token_user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
        let expected_wide: Vec<u16> = expected_sid
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut expected = PSID::default();
        unsafe {
            ConvertStringSidToSidW(PCWSTR(expected_wide.as_ptr()), &mut expected)
                .map_err(|error| format!("解析钉扎 SID 失败: {error}"))?;
        }
        let equal = unsafe { EqualSid(token_user.User.Sid, expected).is_ok() };
        unsafe {
            let _ = LocalFree(Some(HLOCAL(expected.0.cast())));
        }
        Ok(equal)
    })();
    unsafe {
        let _ = CloseHandle(token);
    }
    result
}

#[cfg(windows)]
fn serve_authenticated(pin: &LaunchPin, handle: windows::Win32::Foundation::HANDLE) {
    use std::os::windows::io::{AsRawHandle, FromRawHandle};

    let mut file = unsafe { std::fs::File::from_raw_handle(handle.0) };
    let authorized = match client_matches_user(
        windows::Win32::Foundation::HANDLE(file.as_raw_handle() as *mut _),
        &pin.user_sid,
    ) {
        Ok(true) => true,
        Ok(false) => {
            log_event("pipe client SID does not match pinned user");
            false
        }
        Err(error) => {
            log_event(format!("pipe client identity check failed: {error}"));
            false
        }
    };
    if !authorized {
        let _ = write_frame(&mut file, "E|auth\n");
        return;
    }

    let write = match file.try_clone() {
        Ok(write) => write,
        Err(error) => {
            log_event(format!("clone refresh pipe failed: {error}"));
            return;
        }
    };
    let pinned_game = pin.game_exe.clone();
    let pinned_root = pin.data_root.clone();
    serve_with_launch(file, write, send_f10, move |request, stage| {
        let game = validate_launch_request(request, pin).map_err(str::to_string)?;
        let Some(data_root) = pinned_root.as_deref() else {
            return Err("未钉扎数据根目录".to_string());
        };
        let paths = runtime_paths(data_root);
        let process_name = game
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("StarRail.exe")
            .to_string();
        let options = GameLaunchOptions {
            game_exe: pinned_game.clone().unwrap_or(game),
            migoto_dir: paths.runtime_root,
            injector_dll: paths.injector_dll,
            process_name,
            work_mode: liquimod_core::d3d::MigotoWorkMode::Play,
            delay_ms: 0,
            sync_d3dx_ini: false,
        };
        liquimod_core::launcher::launch_with_mod_progress(&options, stage)
            .map_err(|error| error.to_string())
    });
}

#[cfg(windows)]
fn main() {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE};
    use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
    };

    let _guard = init_tracing();
    let pin = match parse_pin_args(std::env::args().skip(1)) {
        Ok(pin) => pin,
        Err(error) => {
            log_event(format!("invalid helper arguments: {error}"));
            std::process::exit(2);
        }
    };
    log_event(format!("refresh helper starting pipe={PIPE} pin={pin:?}"));
    let security = match create_pipe_security(&pin.user_sid) {
        Ok(security) => security,
        Err(error) => {
            log_event(error);
            std::process::exit(2);
        }
    };
    let wide: Vec<u16> = PIPE.encode_utf16().chain(std::iter::once(0)).collect();
    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(wide.as_ptr()),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            64 * 1024,
            64 * 1024,
            0,
            Some(&security.attributes as *const _),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        log_event("CreateNamedPipeW failed");
        std::process::exit(3);
    }
    let connected = unsafe { ConnectNamedPipe(handle, None) };
    if connected.is_err()
        && connected
            .err()
            .is_none_or(|error| error.code() != ERROR_PIPE_CONNECTED.into())
    {
        log_event("ConnectNamedPipe failed");
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(handle);
        }
        std::process::exit(4);
    }
    log_event("refresh pipe connected");
    serve_authenticated(&pin, handle);
    log_event("refresh pipe disconnected");
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

    #[test]
    fn sddl_is_one_explicit_user_ace_and_rejects_injection() {
        assert_eq!(
            pipe_security_sddl_text("S-1-5-21-123-456-789-1001").unwrap(),
            "D:P(A;;GA;;;S-1-5-21-123-456-789-1001)"
        );
        assert!(pipe_security_sddl_text("S-1-5-21-1|D:(A;;GA;;;WD)").is_err());
        assert!(pipe_security_sddl_text("S-1-5-18").is_err());
    }

    #[test]
    fn launch_request_requires_exact_pinned_canonical_path() {
        let temp = tempfile::tempdir().unwrap();
        let game = temp.path().join("Game.exe");
        std::fs::write(&game, b"exe").unwrap();
        let pin = LaunchPin {
            user_sid: "S-1-5-21-123-456-789-1001".to_string(),
            game_exe: Some(game.clone()),
            data_root: Some(temp.path().to_path_buf()),
        };
        assert_eq!(
            validate_launch_request(&game.display().to_string(), &pin),
            Ok(game.canonicalize().unwrap())
        );
        assert!(validate_launch_request("C:\\other.exe", &pin).is_err());
        assert!(validate_launch_request("bad|path", &pin).is_err());
    }

    #[test]
    fn launch_frames_emit_stage_and_terminal_reply() {
        let mut output = Vec::new();
        serve_with_launch(
            std::io::Cursor::new(b"LAUNCH|ignored.exe\n".to_vec()),
            &mut output,
            |_| true,
            |request, stage| {
                assert_eq!(request, "ignored.exe");
                stage("hook_ok");
                Ok(LaunchResult {
                    success: true,
                    message: "done".to_string(),
                    pid: Some(7),
                })
            },
        );
        assert_eq!(output, b"Shook_ok\nL1|7|done\n");
    }

    #[test]
    fn unsuccessful_launch_result_emits_failure_frame() {
        assert_eq!(
            launch_reply_frame(&LaunchResult {
                success: false,
                message: "spawn failed\nwith details".to_string(),
                pid: Some(7),
            }),
            "L0|spawn failed with details\n"
        );
    }

    #[test]
    fn pin_parser_rejects_missing_duplicate_and_unknown_args() {
        assert!(parse_pin_args(Vec::<String>::new()).is_err());
        assert!(parse_pin_args(vec![
            "--user-sid=S-1-5-21-1-2-3-4".to_string(),
            "--user-sid=S-1-5-21-1-2-3-5".to_string(),
        ])
        .is_err());
        assert!(parse_pin_args(vec![
            "--user-sid=S-1-5-21-1-2-3-4".to_string(),
            "--unknown=value".to_string(),
        ])
        .is_err());
    }

    #[test]
    fn pin_parser_accepts_f10_only_pin() {
        let pin = parse_pin_args(vec!["--user-sid=S-1-5-21-1-2-3-4".to_string()]).unwrap();
        assert_eq!(pin.user_sid, "S-1-5-21-1-2-3-4");
        assert!(pin.game_exe.is_none());
        assert!(pin.data_root.is_none());
    }

    #[test]
    fn malformed_launch_frames_are_rejected_without_launch_callback() {
        let mut output = Vec::new();
        let mut calls = 0;
        serve_with_launch(
            std::io::Cursor::new(b"Lnot-a-launch\n".to_vec()),
            &mut output,
            |_| true,
            |_request, _stage| {
                calls += 1;
                Ok(LaunchResult {
                    success: true,
                    message: "unexpected".to_string(),
                    pid: None,
                })
            },
        );
        assert_eq!(calls, 0);
        assert_eq!(output, b"E|pinned\n");
    }

    #[test]
    fn leading_noise_resynchronizes_to_valid_command() {
        let mut output = Vec::new();
        let mut received = String::new();
        serve(
            std::io::Cursor::new(b"?pStarRail.exe\0".to_vec()),
            &mut output,
            |process| {
                received = process.to_string();
                true
            },
        );
        assert_eq!(received, "StarRail.exe");
        assert_eq!(output, b"1");
    }

    #[test]
    fn oversized_pending_command_is_dropped() {
        let mut output = Vec::new();
        serve(
            std::io::Cursor::new(vec![b'x'; MAX_COMMAND_BYTES + 1]),
            &mut output,
            |_| panic!("oversized noise must not trigger"),
        );
        assert!(output.is_empty());
    }
}
