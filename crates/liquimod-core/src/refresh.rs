//! 游戏刷新与启动:检测游戏进程;经命名管道通知提权 helper 发 F10 或启动游戏。
//! helper 不存在时用 ShellExecuteW "runas" 提权拉起(触发一次 UAC)。
//!
//! 自 v0.7 起主程序以普通权限运行,启动注入完全由提权 helper 执行:
//! helper 启动时经 argv 钉扎用户 SID、游戏路径与数据根目录,管道只放行
//! 该用户,且 helper 拒绝任何偏离钉扎的请求。

use crate::error::{LiquiModError, Result};
use crate::launcher::LaunchResult;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// v6 管道:合并 F10 与 LAUNCH 协议。
/// 避开 v0.6.1 无名端点、v0.6.2/v0.6.3 的 v3/v4 端点以及实验占用的 v5 端点,
/// 使残留旧 helper 不会与新客户端混淆协议。
pub const PIPE_NAME: &str = r"\\.\pipe\liquimod-refresh-v6";
pub const HELPER_EXE: &str = "liquimod-refresh-helper.exe";

const USER_SID_ARG_PREFIX: &str = "--user-sid=";
const GAME_EXE_ARG_PREFIX: &str = "--game-exe=";
const DATA_ROOT_ARG_PREFIX: &str = "--data-root=";

/// helper 被占用(正执行最长约 30 秒的启动注入)时的宽限等待上限。
const PIPE_BUSY_GRACE: Duration = Duration::from_secs(35);
/// 单条回帧缓冲上限,防止异常 helper 撑爆内存。
const MAX_REPLY_BYTES: usize = 64 * 1024;
/// launch 回帧整体超时:覆盖 5s 早期 + 15s 窗口 + 5s 晚期 + spawn/DLL 余量。
const LAUNCH_REPLY_TIMEOUT: Duration = Duration::from_secs(60);

#[cfg(windows)]
const ERROR_PIPE_BUSY: i32 = 231;
#[cfg(windows)]
const ERROR_FILE_NOT_FOUND: i32 = 2;

/// 提权 helper 的启动钉扎:UAC 时刻由客户端确定并经 argv 传给 helper。
/// helper 拒绝偏离钉扎的请求;攻击者无法不经新一次 UAC 改变钉扎。
/// `game_exe` / `data_root` 为 None 时 helper 仅提供 F10(LAUNCH 拒绝),
/// 允许未配置游戏路径的用户继续使用热重载。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPin {
    pub user_sid: String,
    pub game_exe: Option<PathBuf>,
    pub data_root: Option<PathBuf>,
}

/// `launch_game` 的错误分类:helper 明确拒绝时重试/重连无意义。
#[derive(Debug)]
pub enum HelperReplyError {
    /// helper 拒绝请求(身份校验失败或钉扎不匹配),消息含 `E|...` 原始代码。
    Rejected(String),
    /// 启动、注入或 IPC 失败;可按需重连后重试。
    Failed(String),
}

impl std::fmt::Display for HelperReplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(message) | Self::Failed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for HelperReplyError {}

/// 任一给定进程名存在即为游戏运行中(大小写不敏感,免分配比较)。
pub fn is_game_running(process_names: &[&str]) -> bool {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes().values().any(|p| {
        process_names
            .iter()
            .any(|n| p.name().eq_ignore_ascii_case(*n))
    })
}

/// 持有管道双工端 = app 生命周期;Drop 即断开,helper 随之退出。
pub struct RefreshClient {
    pipe: File,
    pin: LaunchPin,
}

impl RefreshClient {
    /// 连接已运行的 helper;否则 runas 提权拉起并等待管道就绪(最多 5s)。
    ///
    /// # 阻塞性
    /// 本方法会阻塞调用线程:可能跨越整个 UAC 弹窗期间,外加最多 5s 的管道轮询。
    /// **必须**从阻塞/工作线程调用(如 `spawn_blocking`),切勿在 async 或主 UI 线程上调用。
    ///
    /// # 单客户端管道
    /// 管道仅支持单一客户端:helper 正忙(可能在执行启动注入)时返回 `ERROR_PIPE_BUSY`,
    /// 此时按 [`PIPE_BUSY_GRACE`] 宽限轮询而不是盲目再次弹 UAC;宽限耗尽返回 `TimedOut`。
    pub fn connect_or_launch(helper_exe: &Path, pin: LaunchPin) -> Result<Self> {
        Self::connect_or_launch_with_grace(helper_exe, pin, PIPE_BUSY_GRACE)
    }

    /// 与 [`RefreshClient::connect_or_launch`] 相同,宽限时长可指定(便于测试)。
    pub fn connect_or_launch_with_grace(
        helper_exe: &Path,
        pin: LaunchPin,
        busy_grace: Duration,
    ) -> Result<Self> {
        Self::connect_on_pipe(helper_exe, pin, busy_grace, PIPE_NAME)
    }

    /// 连接指定端点;公开客户端固定使用 [`PIPE_NAME`],测试可隔离端点并行运行。
    pub(crate) fn connect_on_pipe(
        helper_exe: &Path,
        pin: LaunchPin,
        busy_grace: Duration,
        pipe_name: &str,
    ) -> Result<Self> {
        match Self::try_connect_to(pipe_name) {
            Ok(pipe) => {
                tracing::info!("connected to existing refresh helper pipe");
                return Ok(Self { pipe, pin });
            }
            Err(error) if is_pipe_busy(&error) => {
                // helper 存活但正忙(可能在进行启动注入):宽限轮询等待空闲实例。
                tracing::info!("refresh helper busy, waiting up to {busy_grace:?}");
                if let Some(pipe) = Self::poll_connect(busy_grace, pipe_name) {
                    return Ok(Self { pipe, pin });
                }
                return Err(LiquiModError::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "helper 正忙,等待空闲超时(可能有其他实例正在启动游戏)",
                )));
            }
            Err(error) if is_not_found(&error) => {
                // 没有运行中的 helper:钉扎校验后提权拉起。
                validate_pin(&pin)?;
                launch_elevated(helper_exe, &pin)?;
            }
            Err(error) => {
                // ERROR_ACCESS_DENIED 等异常:盲目重启 helper 只会白弹 UAC,直接报错。
                return Err(LiquiModError::Io(std::io::Error::other(format!(
                    "连接刷新 helper 管道失败: {error}"
                ))));
            }
        }
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(100));
            if let Ok(pipe) = Self::try_connect_to(pipe_name) {
                return Ok(Self { pipe, pin });
            }
        }
        Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "helper 管道等待超时",
        )))
    }

    fn try_connect_to(pipe_name: &str) -> std::io::Result<File> {
        OpenOptions::new().read(true).write(true).open(pipe_name)
    }

    fn poll_connect(deadline: Duration, pipe_name: &str) -> Option<File> {
        let start = std::time::Instant::now();
        while start.elapsed() < deadline {
            std::thread::sleep(Duration::from_millis(200));
            match Self::try_connect_to(pipe_name) {
                Ok(pipe) => return Some(pipe),
                Err(error) if is_pipe_busy(&error) || is_not_found(&error) => continue,
                Err(_) => return None,
            }
        }
        None
    }

    /// 当前客户端使用的钉扎(与 helper argv 钉扎不一致时 helper 会拒绝 LAUNCH)。
    pub fn pin(&self) -> &LaunchPin {
        &self.pin
    }

    /// 通知 helper 针对指定游戏进程窗口发一次 F10。
    /// 协议为 `p<exe-name>\0`,单字节 ack:`1` 成功 / 其他失败。
    /// v6 起不再有 legacy `1` 回退:旧 helper 监听旧端点,回退只会造成 ack 错位。
    pub fn poke_for_process(&mut self, process_name: &str) -> Result<()> {
        let process_name = process_name.trim();
        if process_name.is_empty()
            || process_name.contains('\0')
            || process_name.contains('/')
            || process_name.contains('\\')
        {
            return Err(LiquiModError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "游戏进程名无效",
            )));
        }
        tracing::info!(process = %process_name, "sending F10 refresh request");
        self.pipe.write_all(b"p")?;
        self.pipe.write_all(process_name.as_bytes())?;
        self.pipe.write_all(&[0])?;
        self.pipe.flush()?;
        wait_for_pipe_reply(&self.pipe, Duration::from_secs(2))?;
        let mut ack = [0u8; 1];
        self.pipe.read_exact(&mut ack)?;
        tracing::info!(process = %process_name, ack = ack[0], "refresh helper replied");
        if ack[0] == b'1' {
            Ok(())
        } else {
            Err(LiquiModError::Io(std::io::Error::other(
                "未找到或无法聚焦游戏窗口,F10 未发送",
            )))
        }
    }

    /// 通知 helper 以钉扎的 3DMigoto Hook 流程启动游戏并等待注入完成。
    ///
    /// 发送 `LAUNCH|<game_exe>\n`(钉扎回显,helper 与其 argv 钉扎比对);
    /// 回帧为若干 `S<stage>\n` 进度帧,随后一条终止帧:
    /// `L1|<pid>|<message>` 成功 / `L0|<reason>` 失败 / `E|<code>` 拒绝。
    /// 进度经 `on_stage` 回调上抛;整体超时 [`LAUNCH_REPLY_TIMEOUT`]。
    #[cfg(windows)]
    pub fn launch_game(
        &mut self,
        on_stage: &mut dyn FnMut(&str),
    ) -> std::result::Result<LaunchResult, HelperReplyError> {
        let Some(game_path) = self.pin.game_exe.as_deref() else {
            return Err(HelperReplyError::Failed(
                "未配置游戏主程序路径,无法请求 helper 启动".to_string(),
            ));
        };
        let game = game_path.to_string_lossy();
        if game.is_empty()
            || game
                .chars()
                .any(|character| matches!(character, '\0' | '|' | '\r' | '\n'))
        {
            return Err(HelperReplyError::Failed(
                "游戏路径包含非法字符,无法发送启动请求".to_string(),
            ));
        }
        tracing::info!(game = %game, "sending launch request to refresh helper");
        let command = format!("LAUNCH|{game}\n");
        self.pipe
            .write_all(command.as_bytes())
            .and_then(|()| self.pipe.flush())
            .map_err(|error| HelperReplyError::Failed(format!("启动请求发送失败: {error}")))?;

        let deadline = std::time::Instant::now() + LAUNCH_REPLY_TIMEOUT;
        let mut pending: Vec<u8> = Vec::new();
        loop {
            let available = wait_for_data(&self.pipe, deadline).map_err(|error| {
                HelperReplyError::Failed(format!("等待 helper 启动结果超时: {error}"))
            })?;
            let mut chunk = vec![0u8; available];
            self.pipe
                .read_exact(&mut chunk)
                .map_err(|error| HelperReplyError::Failed(format!("读取启动结果失败: {error}")))?;
            pending.extend_from_slice(&chunk);
            while let Some(end) = pending.iter().position(|byte| *byte == b'\n') {
                let line: Vec<u8> = pending.drain(..=end).collect();
                if let Some(result) = process_launch_line(&line, on_stage) {
                    return result;
                }
            }
            if pending.len() > MAX_REPLY_BYTES {
                return Err(HelperReplyError::Failed(
                    "helper 回帧超过大小限制,协议异常".to_string(),
                ));
            }
        }
    }

    #[cfg(not(windows))]
    pub fn launch_game(
        &mut self,
        _on_stage: &mut dyn FnMut(&str),
    ) -> std::result::Result<LaunchResult, HelperReplyError> {
        Err(HelperReplyError::Failed(
            "仅 Windows 支持提权启动注入".to_string(),
        ))
    }
}

/// 解析一行 helper 回帧;产出终止结果时返回 `Some`。
fn process_launch_line(
    line: &[u8],
    on_stage: &mut dyn FnMut(&str),
) -> Option<std::result::Result<LaunchResult, HelperReplyError>> {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    if line.is_empty() {
        return None;
    }
    let Ok(text) = std::str::from_utf8(line) else {
        return Some(Err(HelperReplyError::Failed(
            "helper 回帧不是有效 UTF-8,协议异常".to_string(),
        )));
    };
    if let Some(stage) = text.strip_prefix('S') {
        if !stage.is_empty() && !stage.contains('|') {
            on_stage(stage);
            return None;
        }
    }
    if let Some(payload) = text.strip_prefix("L1|") {
        let mut parts = payload.splitn(2, '|');
        let pid = parts.next().and_then(|value| {
            (!value.is_empty())
                .then(|| value.parse::<u32>().ok())
                .flatten()
        });
        let message = parts.next().unwrap_or("XXMI Hook 已完成").to_string();
        return Some(Ok(LaunchResult {
            success: true,
            message,
            pid,
        }));
    }
    if let Some(reason) = text.strip_prefix("L0|") {
        return Some(Err(HelperReplyError::Failed(format!(
            "游戏启动或 3DMigoto 注入失败:{reason}"
        ))));
    }
    if let Some(code) = text.strip_prefix("E|") {
        return Some(Err(HelperReplyError::Rejected(format!(
            "helper 拒绝了启动请求({code})"
        ))));
    }
    Some(Err(HelperReplyError::Failed(format!(
        "helper 回帧无法识别:{text}"
    ))))
}

/// 校验钉扎字段:格式、存在性与类型在拉起 helper 前检查,fail-closed。
fn validate_pin(pin: &LaunchPin) -> Result<()> {
    if !is_valid_user_sid_text(&pin.user_sid) {
        return Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "当前用户 SID 格式无效,无法启动提权 helper",
        )));
    }
    let invalid_chars = |path: &Path| {
        let value = path.to_string_lossy();
        value.is_empty()
            || !path.is_absolute()
            || value
                .chars()
                .any(|character| matches!(character, '\0' | '"'))
    };
    if let Some(game_exe) = &pin.game_exe {
        if invalid_chars(game_exe)
            || !game_exe.is_file()
            || game_exe
                .extension()
                .and_then(|ext| ext.to_str())
                .is_none_or(|ext| !ext.eq_ignore_ascii_case("exe"))
        {
            return Err(LiquiModError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("helper 钉扎的游戏路径无效: {}", game_exe.display()),
            )));
        }
    }
    if let Some(data_root) = &pin.data_root {
        if invalid_chars(data_root) || !data_root.is_dir() {
            return Err(LiquiModError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("helper 钉扎的数据根目录无效: {}", data_root.display()),
            )));
        }
    }
    if pin.game_exe.is_some() != pin.data_root.is_some() {
        return Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "游戏路径与数据根目录必须成对提供",
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn is_pipe_busy(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(ERROR_PIPE_BUSY)
}

#[cfg(not(windows))]
fn is_pipe_busy(_error: &std::io::Error) -> bool {
    false
}

#[cfg(windows)]
fn is_not_found(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(ERROR_FILE_NOT_FOUND)
}

#[cfg(not(windows))]
fn is_not_found(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::NotFound
}

#[cfg(windows)]
fn peek_pipe_available(pipe: &File) -> std::io::Result<u32> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Pipes::PeekNamedPipe;

    let mut available = 0;
    let handle = HANDLE(pipe.as_raw_handle());
    unsafe { PeekNamedPipe(handle, None, 0, None, Some(&mut available), None) }
        .map_err(std::io::Error::other)?;
    Ok(available)
}

#[cfg(windows)]
fn wait_for_data(pipe: &File, deadline: std::time::Instant) -> std::io::Result<usize> {
    loop {
        let available = peek_pipe_available(pipe)?;
        if available > 0 {
            return Ok(available as usize);
        }
        if std::time::Instant::now() >= deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "helper 响应超时",
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(windows)]
fn wait_for_pipe_reply(pipe: &File, timeout: Duration) -> std::io::Result<()> {
    let deadline = std::time::Instant::now() + timeout;
    wait_for_data(pipe, deadline).map(|_| ())
}

#[cfg(not(windows))]
fn wait_for_pipe_reply(_pipe: &File, _timeout: Duration) -> std::io::Result<()> {
    Ok(())
}

/// 获取当前进程的用户 SID 文本,交给 helper 为该用户建立最小权限管道 ACL。
#[cfg(windows)]
pub fn current_user_sid() -> Result<String> {
    use std::mem::size_of;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
    use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token = HANDLE::default();
    unsafe {
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
            .map_err(|error| LiquiModError::Io(std::io::Error::other(error)))?;
    }

    let result = (|| {
        let mut required = 0u32;
        let _ = unsafe {
            GetTokenInformation(token, TokenUser, None, 0, std::ptr::addr_of_mut!(required))
        };
        if required == 0 {
            return Err(LiquiModError::Io(std::io::Error::other(
                "获取当前用户 SID 所需缓冲区大小失败",
            )));
        }

        // 以 usize 对齐底层缓冲区,避免将 TOKEN_USER 从未对齐的 u8 缓冲区读取。
        let units = (required as usize).div_ceil(size_of::<usize>());
        let mut buffer = vec![0usize; units];
        unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                Some(buffer.as_mut_ptr().cast()),
                (buffer.len() * size_of::<usize>()) as u32,
                std::ptr::addr_of_mut!(required),
            )
            .map_err(|error| LiquiModError::Io(std::io::Error::other(error)))?;
        }

        let token_user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
        let mut sid_text = PWSTR::null();
        unsafe {
            ConvertSidToStringSidW(token_user.User.Sid, &mut sid_text)
                .map_err(|error| LiquiModError::Io(std::io::Error::other(error)))?;
        }
        let sid = unsafe { sid_text.to_string() }
            .map_err(|error| LiquiModError::Io(std::io::Error::other(error)))?;
        unsafe {
            let _ = LocalFree(Some(HLOCAL(sid_text.0.cast())));
        }
        if !is_valid_user_sid_text(&sid) {
            return Err(LiquiModError::Io(std::io::Error::other(
                "当前用户 SID 格式无效",
            )));
        }
        Ok(sid)
    })();

    unsafe {
        let _ = CloseHandle(token);
    }
    result
}

#[cfg(not(windows))]
pub fn current_user_sid() -> Result<String> {
    Err(std::io::Error::other("仅 Windows 支持用户 SID").into())
}

/// 只接受普通域用户 SID(`S-1-5-21-*`),拒绝 SYSTEM/服务账户与注入字符。
pub fn is_valid_user_sid_text(value: &str) -> bool {
    value.len() > 9
        && value.starts_with("S-1-5-21-")
        && value
            .chars()
            .skip(9)
            .all(|character| character.is_ascii_digit() || character == '-')
        && !value.ends_with('-')
}

/// ShellExecuteW(runas) 提权启动 helper,并传入 argv 钉扎。
/// 路径参数加引号以容忍空格;钉扎字段已在上游校验不含引号与控制字符。
#[cfg(windows)]
fn launch_elevated(exe: &Path, pin: &LaunchPin) -> Result<()> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD;

    let runas: Vec<u16> = "runas\0".encode_utf16().collect();
    let path: Vec<u16> = format!("{}\0", exe.display()).encode_utf16().collect();
    let mut parameters = format!("{USER_SID_ARG_PREFIX}{}", pin.user_sid);
    if let Some(game_exe) = &pin.game_exe {
        parameters.push_str(&format!(" {GAME_EXE_ARG_PREFIX}\"{}\"", game_exe.display()));
    }
    if let Some(data_root) = &pin.data_root {
        parameters.push_str(&format!(
            " {DATA_ROOT_ARG_PREFIX}\"{}\"",
            data_root.display()
        ));
    }
    let parameters: Vec<u16> = format!("{parameters}\0").encode_utf16().collect();
    let r = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(runas.as_ptr()),
            PCWSTR(path.as_ptr()),
            PCWSTR(parameters.as_ptr()),
            PCWSTR::null(),
            SHOW_WINDOW_CMD(1), // SW_SHOWNORMAL
        )
    };
    if r.0 as usize > 32 {
        Ok(())
    } else {
        Err(LiquiModError::Io(std::io::Error::other(format!(
            "helper 启动失败(可能拒绝了 UAC),code {}",
            r.0 as usize
        ))))
    }
}

#[cfg(not(windows))]
fn launch_elevated(_exe: &Path, _pin: &LaunchPin) -> Result<()> {
    Err(LiquiModError::Io(std::io::Error::other(
        "仅 Windows 支持刷新 helper",
    )))
}

/// 启动外部可执行文件:
/// 1. 优先使用 ShellExecuteW("open") 启动,工作目录设在 exe 所在文件夹;
/// 2. 若遇 SE_ERR_ACCESSDENIED=5,自动以 "runas" 动词请求 UAC 提权;
/// 3. 若启动成功返回 Ok(()),若用户取消或文件异常返回清晰错误。
#[cfg(windows)]
pub fn launch_program(exe: &Path) -> Result<()> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD;

    let dir = exe.parent().unwrap_or_else(|| Path::new("."));
    let open_verb: Vec<u16> = "open\0".encode_utf16().collect();
    let runas_verb: Vec<u16> = "runas\0".encode_utf16().collect();
    let file_path: Vec<u16> = format!("{}\0", exe.display()).encode_utf16().collect();
    let dir_path: Vec<u16> = format!("{}\0", dir.display()).encode_utf16().collect();

    let r = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(open_verb.as_ptr()),
            PCWSTR(file_path.as_ptr()),
            PCWSTR::null(),
            PCWSTR(dir_path.as_ptr()),
            SHOW_WINDOW_CMD(1), // SW_SHOWNORMAL
        )
    };

    if r.0 as usize > 32 {
        return Ok(());
    }

    if r.0 as usize == 5 {
        let r_runas = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(runas_verb.as_ptr()),
                PCWSTR(file_path.as_ptr()),
                PCWSTR::null(),
                PCWSTR(dir_path.as_ptr()),
                SHOW_WINDOW_CMD(1),
            )
        };
        if r_runas.0 as usize > 32 {
            return Ok(());
        }
        return Err(LiquiModError::Io(std::io::Error::other(format!(
            "程序启动被拒绝或未授权管理员权限 (code {})",
            r_runas.0 as usize
        ))));
    }

    Err(LiquiModError::Io(std::io::Error::other(format!(
        "启动程序失败「{}」(code {})",
        exe.display(),
        r.0 as usize
    ))))
}

#[cfg(not(windows))]
pub fn launch_program(exe: &Path) -> Result<()> {
    let dir = exe.parent().unwrap_or_else(|| Path::new("."));
    std::process::Command::new(exe)
        .current_dir(dir)
        .spawn()
        .map_err(LiquiModError::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_running_process_case_insensitive() {
        let own = std::env::current_exe()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_lowercase();
        // 测试 harness 进程名形如 liquimod_core-xxxx.exe,取不含 hash 的前缀不可行,
        // 直接断言:不存在的进程 false;当前进程 true。
        assert!(!is_game_running(&["definitely-not-running-zzz.exe"]));
        assert!(is_game_running(&[&own]));
    }

    #[test]
    fn accepts_domain_user_sid_text_only() {
        assert!(is_valid_user_sid_text("S-1-5-21-123-456-789-1001"));
        assert!(!is_valid_user_sid_text("S-1-5-18")); // SYSTEM
        assert!(!is_valid_user_sid_text("S-1-5-21-123-456-789-1001|bad"));
        assert!(!is_valid_user_sid_text("S-1-5-21-"));
        assert!(!is_valid_user_sid_text("Administrators"));
        assert!(!is_valid_user_sid_text("S-1-5-21-123-456-789-1001\""));
    }

    #[test]
    fn pin_requires_game_and_data_root_together() {
        let game = std::env::current_exe().unwrap();
        let valid = LaunchPin {
            user_sid: "S-1-5-21-123-456-789-1001".to_string(),
            game_exe: Some(game.clone()),
            data_root: Some(std::env::temp_dir()),
        };
        assert!(validate_pin(&valid).is_ok());
        assert!(validate_pin(&LaunchPin {
            data_root: valid.data_root.clone(),
            game_exe: None,
            ..valid.clone()
        })
        .is_err());
        assert!(validate_pin(&LaunchPin {
            game_exe: Some(game),
            data_root: None,
            ..valid
        })
        .is_err());
        assert!(validate_pin(&LaunchPin {
            user_sid: "S-1-5-21-123-456-789-1001".to_string(),
            game_exe: None,
            data_root: None,
        })
        .is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn current_user_sid_returns_domain_user_sid() {
        let sid = current_user_sid().unwrap();
        assert!(is_valid_user_sid_text(&sid), "unexpected SID: {sid}");
    }

    #[test]
    fn launch_line_parses_stage_success_failure_and_rejection() {
        let stages = std::cell::RefCell::new(Vec::new());
        let mut on_stage = |stage: &str| stages.borrow_mut().push(stage.to_string());

        assert!(process_launch_line(b"Shook_ok\n", &mut on_stage).is_none());
        assert!(process_launch_line(b"Swindow_seen\n", &mut on_stage).is_none());
        assert!(process_launch_line(b"\n", &mut on_stage).is_none());
        assert_eq!(
            stages.borrow().as_slice(),
            &["hook_ok".to_string(), "window_seen".to_string()]
        );

        let ok = process_launch_line("L1|12345|XXMI Hook 已完成\n".as_bytes(), &mut on_stage)
            .expect("terminal frame")
            .expect("success frame");
        assert!(ok.success);
        assert_eq!(ok.pid, Some(12345));
        assert_eq!(ok.message, "XXMI Hook 已完成");

        let ok_no_pid = process_launch_line("L1||XXMI Hook 已完成\n".as_bytes(), &mut on_stage)
            .expect("terminal frame")
            .expect("success frame");
        assert_eq!(ok_no_pid.pid, None);

        let failed = process_launch_line(b"L0|spawn failed\n", &mut on_stage)
            .expect("terminal frame")
            .expect_err("failure frame");
        assert!(matches!(failed, HelperReplyError::Failed(_)));

        let rejected = process_launch_line(b"E|pinned\n", &mut on_stage)
            .expect("terminal frame")
            .expect_err("rejection frame");
        assert!(matches!(rejected, HelperReplyError::Rejected(_)));

        let garbage = process_launch_line(b"XYZZY\n", &mut on_stage)
            .expect("terminal frame")
            .expect_err("protocol error");
        assert!(matches!(garbage, HelperReplyError::Failed(_)));
    }

    /// 在指定管道名上创建一个单实例服务端,用于 busy 宽限与端到端帧测试。
    /// 每个测试使用独立端点,避免并行用例争抢单实例管道。
    #[cfg(windows)]
    struct TestPipeServer {
        name: String,
        handle: windows::Win32::Foundation::HANDLE,
    }

    #[cfg(windows)]
    impl TestPipeServer {
        fn new(name: &str) -> Self {
            use windows::core::PCWSTR;
            use windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
            use windows::Win32::System::Pipes::{CreateNamedPipeW, PIPE_TYPE_BYTE, PIPE_WAIT};

            let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let handle = unsafe {
                CreateNamedPipeW(
                    PCWSTR(wide.as_ptr()),
                    PIPE_ACCESS_DUPLEX,
                    PIPE_TYPE_BYTE | PIPE_WAIT,
                    1,
                    0,
                    0,
                    0,
                    None,
                )
            };
            assert_ne!(
                handle,
                windows::Win32::Foundation::INVALID_HANDLE_VALUE,
                "test pipe creation failed"
            );
            Self {
                name: name.to_string(),
                handle,
            }
        }

        /// 模拟 helper 正被客户端占用:连上唯一实例,使新连接得到 ERROR_PIPE_BUSY。
        fn occupy(&self) -> File {
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.name)
                .unwrap()
        }
    }

    #[cfg(windows)]
    impl Drop for TestPipeServer {
        fn drop(&mut self) {
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(self.handle);
            }
        }
    }

    #[cfg(windows)]
    fn test_pin() -> LaunchPin {
        LaunchPin {
            user_sid: "S-1-5-21-123-456-789-1001".to_string(),
            game_exe: Some(std::env::current_exe().unwrap()),
            data_root: Some(std::env::temp_dir()),
        }
    }

    #[cfg(windows)]
    #[test]
    fn busy_pipe_beyond_grace_times_out_without_launch() {
        let server = TestPipeServer::new(r"\\.\pipe\liquimod-refresh-test-busy");
        let _occupant = server.occupy();

        let result = RefreshClient::connect_on_pipe(
            // 若误判为"无 helper"会尝试 runas 拉起该路径并立即失败;
            // 用不存在的路径确保测试能发现错误的拉起行为。
            Path::new("C:\\definitely-missing-helper.exe"),
            test_pin(),
            Duration::from_millis(300),
            &server.name,
        );

        let error = result.err().expect("busy pipe must not connect");
        assert!(matches!(error, LiquiModError::Io(_)));
        assert!(error.to_string().contains("正忙"));
    }

    #[cfg(windows)]
    #[test]
    fn launch_game_reads_staged_frames_over_real_pipe() {
        use windows::Win32::Storage::FileSystem::{ReadFile, WriteFile};
        use windows::Win32::System::Pipes::ConnectNamedPipe;

        let server = TestPipeServer::new(r"\\.\pipe\liquimod-refresh-test-e2e");
        let raw_handle = server.handle.0 as usize;
        let writer = std::thread::spawn(move || {
            let handle = windows::Win32::Foundation::HANDLE(raw_handle as *mut _);
            unsafe {
                let _ = ConnectNamedPipe(handle, None);
                // 必须先读掉客户端发来的 LAUNCH 帧:File::flush() 在命名管道上
                // 即 FlushFileBuffers,会阻塞到对端把数据读走;只写不读必死锁。
                let mut request = [0u8; 1024];
                let mut request_len = 0u32;
                let read_ok = ReadFile(handle, Some(&mut request), Some(&mut request_len), None);
                assert!(read_ok.is_ok(), "mock server must read the LAUNCH frame");
                assert!(
                    request[..request_len as usize].starts_with(b"LAUNCH|"),
                    "unexpected client frame"
                );
                let data =
                    "Shook_ok\nSspawned\nSearly_ok\nSwindow_seen\nSlate_ok\nSunhook\nL1|4321|XXMI Hook 已完成\n"
                        .as_bytes();
                let mut written = 0u32;
                let _ = WriteFile(handle, Some(data), Some(&mut written), None);
            }
            // 保持服务端句柄打开,待客户端读完整段回帧(见 join)。
        });

        let mut client = RefreshClient::connect_on_pipe(
            Path::new("unused.exe"),
            test_pin(),
            Duration::from_secs(2),
            &server.name,
        )
        .unwrap();
        let mut stages = Vec::new();
        let result = client
            .launch_game(&mut |stage| stages.push(stage.to_string()))
            .unwrap();
        writer.join().unwrap();

        assert_eq!(
            stages,
            vec![
                "hook_ok",
                "spawned",
                "early_ok",
                "window_seen",
                "late_ok",
                "unhook"
            ]
        );
        assert!(result.success);
        assert_eq!(result.pid, Some(4321));
    }
}

/// 游戏运行状态看门狗:低频轮询进程生命周期,只在状态变化时回调。
pub struct GameWatchdog {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl GameWatchdog {
    pub fn start<F>(process_names: Vec<String>, interval: Duration, mut on_change: F) -> Self
    where
        F: FnMut(bool) + Send + 'static,
    {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let join = thread::spawn(move || {
            let mut last = None;
            while !stop_thread.load(Ordering::Relaxed) {
                let names: Vec<&str> = process_names.iter().map(String::as_str).collect();
                let running = is_game_running(&names);
                if last != Some(running) {
                    on_change(running);
                    last = Some(running);
                }

                let mut elapsed = Duration::ZERO;
                while elapsed < interval && !stop_thread.load(Ordering::Relaxed) {
                    let slice = (interval - elapsed).min(Duration::from_millis(100));
                    thread::sleep(slice);
                    elapsed += slice;
                }
            }
        });
        Self {
            stop,
            join: Some(join),
        }
    }
}

impl Drop for GameWatchdog {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[cfg(test)]
mod watchdog_tests {
    use super::GameWatchdog;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn watchdog_reports_initial_state_and_stops_cleanly() {
        let states = Arc::new(Mutex::new(Vec::new()));
        let observed = Arc::clone(&states);
        let watchdog = GameWatchdog::start(
            vec!["liquimod-process-that-does-not-exist.exe".to_string()],
            Duration::from_millis(20),
            move |running| observed.lock().unwrap().push(running),
        );
        std::thread::sleep(Duration::from_millis(60));
        drop(watchdog);
        assert_eq!(states.lock().unwrap().as_slice(), &[false]);
    }
}
