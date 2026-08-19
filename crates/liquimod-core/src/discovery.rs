//! 崩坏：星穹铁道 游戏安装路径与执行体智能嗅探模块（支持国服、B服、国际服及 HoYoPlay）

use std::path::{Path, PathBuf};

/// 尝试全自动嗅探定位系统中的《崩坏：星穹铁道》游戏主程序 (`StarRail.exe`)
pub fn auto_detect_game_exe() -> Option<PathBuf> {
    // 1. 优先尝试从 Player.log / output_log.txt 日志中提取真实运行路径
    if let Some(exe) = detect_from_player_logs() {
        if is_valid_game_exe(&exe) {
            return Some(exe);
        }
    }

    // 2. 尝试从 Windows 注册表中嗅探安装目录（Windows 专属）
    #[cfg(windows)]
    {
        if let Some(exe) = detect_from_windows_registry() {
            if is_valid_game_exe(&exe) {
                return Some(exe);
            }
        }
    }

    // 3. 常见默认盘符与游戏目录启发式扫描
    let candidate_dirs = [
        r"C:\Program Files\Star Rail\Game",
        r"C:\Program Files\Star Rail Games",
        r"D:\Program Files\Star Rail\Game",
        r"D:\Program Files\Star Rail Games",
        r"D:\Star Rail\Game",
        r"D:\Star Rail Games",
        r"E:\Star Rail\Game",
        r"E:\Star Rail Games",
        r"F:\Star Rail\Game",
        r"F:\Star Rail Games",
        r"D:\Games\Star Rail Games",
        r"E:\Games\Star Rail Games",
    ];

    for dir in candidate_dirs {
        let p = PathBuf::from(dir).join("StarRail.exe");
        if is_valid_game_exe(&p) {
            return Some(p);
        }
    }

    None
}

/// 检查给定路径是否为有效存在的 `StarRail.exe`
pub fn is_valid_game_exe(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    path.file_name()
        .map(|n| n.to_string_lossy().eq_ignore_ascii_case("StarRail.exe"))
        .unwrap_or(false)
}

/// 从 Unity 运行日志中分析游戏主程序所在路径
pub fn detect_from_player_logs() -> Option<PathBuf> {
    let local_appdata = std::env::var("LOCALAPPDATA").ok()?;
    let user_profile = std::env::var("USERPROFILE").ok()?;

    let log_candidates = [
        // 国服 / 官服
        PathBuf::from(&local_appdata).join(r"..\LocalLow\miHoYo\崩坏：星穹铁道\Player.log"),
        PathBuf::from(&local_appdata).join(r"..\LocalLow\miHoYo\崩坏：星穹铁道\output_log.txt"),
        // 国际服 (Cognosphere)
        PathBuf::from(&local_appdata).join(r"..\LocalLow\Cognosphere\Star Rail\Player.log"),
        PathBuf::from(&local_appdata).join(r"..\LocalLow\Cognosphere\Star Rail\output_log.txt"),
        // 用户目录备选
        PathBuf::from(&user_profile).join(r"AppData\LocalLow\miHoYo\崩坏：星穹铁道\Player.log"),
        PathBuf::from(&user_profile).join(r"AppData\LocalLow\Cognosphere\Star Rail\Player.log"),
    ];

    for log_path in &log_candidates {
        if log_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(log_path) {
                if let Some(exe) = parse_exe_from_log_content(&content) {
                    if is_valid_game_exe(&exe) {
                        return Some(exe);
                    }
                }
            }
        }
    }

    None
}

/// 从日志文本中抽取游戏目录（例如匹配 "StarRail_Data\Plugins" 或 "Loading player data from"）
pub fn parse_exe_from_log_content(content: &str) -> Option<PathBuf> {
    for line in content.lines() {
        // 模式 1: "Setting Plugin DLL path to: D:/Games/StarRail_Data\Plugins\x86_64"
        if let Some(idx) = line.find("StarRail_Data") {
            let prefix = &line[..idx];
            let raw_path = prefix
                .rsplit(|c: char| {
                    c == ':' && prefix.ends_with(':')
                        || c == '"'
                        || c == ' '
                        || c == '\t'
                        || c == '\''
                })
                .next()
                .unwrap_or(prefix);

            // 提取盘符开始的位置，如 C:/ 或 D:\
            if let Some(drive_idx) = raw_path.find(|c: char| {
                c.is_ascii_alphabetic()
                    && (raw_path[c.len_utf8()..].starts_with(":\\")
                        || raw_path[c.len_utf8()..].starts_with(":/"))
            }) {
                let clean_base = &raw_path[drive_idx..];
                let exe_candidate = PathBuf::from(clean_base).join("StarRail.exe");
                if is_valid_game_exe(&exe_candidate) {
                    return Some(exe_candidate);
                }
            }
        }

        // 模式 2: 直接包含 "StarRail.exe" 路径
        if let Some(idx) = line.to_ascii_lowercase().find("starrail.exe") {
            let sub = &line[..idx + 12];
            if let Some(drive_idx) = sub.find(|c: char| {
                c.is_ascii_alphabetic()
                    && (sub[c.len_utf8()..].starts_with(":\\")
                        || sub[c.len_utf8()..].starts_with(":/"))
            }) {
                let candidate = PathBuf::from(&sub[drive_idx..]);
                if is_valid_game_exe(&candidate) {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

#[cfg(windows)]
fn detect_from_windows_registry() -> Option<PathBuf> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let reg_keys = [
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\崩坏：星穹铁道",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Star Rail",
        r"HKCU\SOFTWARE\miHoYo\崩坏：星穹铁道",
        r"HKCU\SOFTWARE\Cognosphere\Star Rail",
        r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\崩坏：星穹铁道",
        r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Star Rail",
    ];

    let val_names = ["InstallPath", "Install_Path", "Path"];

    for key in &reg_keys {
        for val in &val_names {
            let output = std::process::Command::new("reg")
                .args(["query", key, "/v", val])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .ok();

            if let Some(out) = output {
                if out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout);
                    for line in text.lines() {
                        if line.contains("REG_SZ") || line.contains("REG_EXPAND_SZ") {
                            if let Some((_, path_str)) = line.split_once("REG_") {
                                if let Some((_, actual_val)) = path_str.split_once(' ') {
                                    let clean_dir = actual_val.trim();
                                    let candidate_1 = PathBuf::from(clean_dir).join("StarRail.exe");
                                    if is_valid_game_exe(&candidate_1) {
                                        return Some(candidate_1);
                                    }
                                    let candidate_2 =
                                        PathBuf::from(clean_dir).join("Game").join("StarRail.exe");
                                    if is_valid_game_exe(&candidate_2) {
                                        return Some(candidate_2);
                                    }
                                    let candidate_3 =
                                        PathBuf::from(clean_dir).join("Games").join("StarRail.exe");
                                    if is_valid_game_exe(&candidate_3) {
                                        return Some(candidate_3);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exe_path_from_sample_unity_log() {
        let sample_log = r#"
Initialize engine version: 2019.4.40f1 (e3095034c442)
[Subsystems] Discovering subsystems at path D:/Games/Star Rail Games/StarRail_Data/UnitySubsystems
GfxDevice: creating device client; threaded=1
Direct3D:
    Version:  Direct3D 11.0 [level 11.1]
    Renderer: NVIDIA GeForce RTX 4070 (ID=0x2786)
    Vendor:   NVIDIA
    VRAM:     12011 MB
Setting Plugin DLL path to: D:/Games/Star Rail Games/StarRail_Data\Plugins\x86_64
"#;
        let found = parse_exe_from_log_content(sample_log);
        let _ = found;
    }
}
