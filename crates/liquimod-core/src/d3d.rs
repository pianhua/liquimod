//! 3Dmigoto 配置探测与 d3dx.ini 解析：从 3Dmigoto 根目录提取游戏路径、加载器路径与 Mods 目录。

use crate::error::{LiquiModError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigotoInfo {
    /// 3Dmigoto 根目录（包含 d3dx.ini 的目录）
    pub root: PathBuf,
    /// d3dx.ini 文件的绝对路径
    pub ini_path: PathBuf,
    /// 游戏主程序 exe 路径（如 D:\Star Rail\Game\StarRail.exe）
    pub game_exe: Option<PathBuf>,
    /// 3Dmigoto 加载器 exe 路径（如 E:\all in\SRMI\3DMigoto Loader.exe）
    pub loader_exe: Option<PathBuf>,
    /// Mods 存放目录（如 E:\all in\SRMI\Mods）
    pub mods_dir: Option<PathBuf>,
}

/// 简易轻量、健壮的 INI 解析：
/// 支持分号 `;` 或 `#` 开头的注释、大小写不敏感的 section 匹配、去除引号及两端空白。
pub fn parse_ini_sections(content: &str) -> HashMap<String, HashMap<String, String>> {
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current_section = String::new();

    for line in content.lines() {
        let mut line_str = line.trim();
        if line_str.is_empty() || line_str.starts_with(';') || line_str.starts_with('#') {
            continue;
        }
        // 剥离分号与井号行内注释（如果不在引号内）
        if let Some((clean, _)) = line_str.split_once(';') {
            line_str = clean.trim();
        }
        if let Some((clean, _)) = line_str.split_once('#') {
            line_str = clean.trim();
        }
        if line_str.is_empty() {
            continue;
        }

        if line_str.starts_with('[') && line_str.ends_with(']') {
            let sec_name = line_str[1..line_str.len() - 1].trim().to_lowercase();
            current_section = sec_name;
            sections.entry(current_section.clone()).or_default();
            continue;
        }
        if let Some((k, v)) = line_str.split_once('=') {
            let key = k.trim().to_lowercase();
            let mut val = v.trim();
            // 剥离可能存在的包裹引号
            if (val.starts_with('"') && val.ends_with('"') && val.len() >= 2)
                || (val.starts_with('\'') && val.ends_with('\'') && val.len() >= 2)
            {
                val = &val[1..val.len() - 1];
            }
            sections
                .entry(current_section.clone())
                .or_default()
                .insert(key, val.to_string());
        }
    }

    sections
}

/// 解析 `d3dx.ini` 文本内容并结合所在根目录构造 `MigotoInfo`。
pub fn parse_d3dx_ini(content: &str, migoto_root: &Path) -> MigotoInfo {
    let sections = parse_ini_sections(content);
    let ini_path = migoto_root.join("d3dx.ini");

    // 1. 查找 target 游戏主程序
    let loader_sec = sections.get("loader");
    let mut game_exe: Option<PathBuf> = None;
    if let Some(sec) = loader_sec {
        if let Some(target_str) = sec.get("target") {
            if !target_str.is_empty() {
                let p = PathBuf::from(target_str);
                if p.is_absolute() {
                    game_exe = Some(p);
                } else {
                    game_exe = Some(migoto_root.join(p));
                }
            }
        }
    }

    // 2. 查找 loader 加载器 exe
    let mut loader_exe: Option<PathBuf> = None;
    if let Some(sec) = loader_sec {
        if let Some(loader_str) = sec.get("loader") {
            if !loader_str.is_empty() {
                let p = PathBuf::from(loader_str);
                let full = if p.is_absolute() {
                    p
                } else {
                    migoto_root.join(p)
                };
                if full.exists() {
                    loader_exe = Some(full);
                }
            }
        }
    }
    // 若 ini 里的 loader 路径不存在，或未配置，尝试在 root 目录下寻找常见 loader 名称
    if loader_exe.is_none() {
        for candidate in [
            "3DMigoto Loader.exe",
            "3DMigotoLoader.exe",
            "3dmigoto loader.exe",
            "3dmigoto.exe",
            "Loader.exe",
        ] {
            let p = migoto_root.join(candidate);
            if p.is_file() {
                loader_exe = Some(p);
                break;
            }
        }
    }

    // 3. 查找 Mods 目录
    let mut mods_dir: Option<PathBuf> = None;
    if let Some(include_sec) = sections.get("include") {
        if let Some(inc_rec) = include_sec.get("include_recursive") {
            if !inc_rec.is_empty() {
                let p = migoto_root.join(inc_rec);
                mods_dir = Some(p);
            }
        }
    }
    // 缺省回退为 root / Mods
    if mods_dir.is_none() {
        let fallback = migoto_root.join("Mods");
        mods_dir = Some(fallback);
    }

    MigotoInfo {
        root: migoto_root.to_path_buf(),
        ini_path,
        game_exe,
        loader_exe,
        mods_dir,
    }
}

/// 探测给定的目录：
/// 1. 若用户直接传入了 3Dmigoto 根目录（包含 `d3dx.ini`），则直接解析；
/// 2. 若用户传入的是 `Mods` 目录，尝试检查其父目录是否包含 `d3dx.ini`；
/// 3. 若 `d3dx.ini` 不存在，返回 `LiquiModError::Io` 错误。
pub fn inspect_migoto_dir(target_dir: &Path) -> Result<MigotoInfo> {
    if !target_dir.is_dir() {
        return Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("目录不存在：{}", target_dir.display()),
        )));
    }

    let mut candidate_root = target_dir.to_path_buf();
    let mut ini_file = candidate_root.join("d3dx.ini");

    if !ini_file.is_file() {
        // 尝试检查父目录（例如用户选择了 E:\all in\SRMI\Mods）
        if let Some(parent) = target_dir.parent() {
            let parent_ini = parent.join("d3dx.ini");
            if parent_ini.is_file() {
                candidate_root = parent.to_path_buf();
                ini_file = parent_ini;
            }
        }
    }

    if !ini_file.is_file() {
        return Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "未在「{}」或其父目录找到 3Dmigoto 配置文件 (d3dx.ini)",
                target_dir.display()
            ),
        )));
    }

    let content = std::fs::read_to_string(&ini_file)?;
    let mut info = parse_d3dx_ini(&content, &candidate_root);

    // 确保绝对路径与规范化（如果文件存在）
    if let Ok(c) = candidate_root.canonicalize() {
        info.root = c;
    }
    if let Ok(c) = ini_file.canonicalize() {
        info.ini_path = c;
    }

    Ok(info)
}

/// 3Dmigoto 工作模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigotoWorkMode {
    /// 🎮 游玩模式：hunting = 0, calls = 0, show_warnings = 0 (极致流畅纯净、零GPU/CPU多余开销、无dump垃圾产生)
    Play,
    /// 🛠️ 抓取开发模式：hunting = 2, marking_actions = clipboard hlsl asm regex (支持小键盘按键实时抓取并复制Hash到剪贴板)
    Dev,
}

/// 探测指定 d3dx.ini 内容当前所处的模式
pub fn inspect_work_mode(content: &str) -> MigotoWorkMode {
    let sections = parse_ini_sections(content);
    if let Some(sec) = sections.get("hunting") {
        if let Some(val) = sec.get("hunting") {
            let v = val.trim();
            if v == "2" || v == "1" {
                return MigotoWorkMode::Dev;
            }
        }
    }
    MigotoWorkMode::Play
}

/// 将目标模式的键值参数精准应用/替换到 d3dx.ini 文本中，同时保留文件中原有的注释、空行和其他配置
pub fn apply_work_mode(content: &str, mode: MigotoWorkMode) -> String {
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

    // 目标参数集：
    // [Hunting]
    //   hunting = 0 (Play) / 2 (Dev)
    //   marking_actions = clipboard (Play) / clipboard hlsl asm regex (Dev)
    // [Logging]
    //   calls = 0 (Play) / 0 (Dev)
    //   debug = 0 (Play) / 0 (Dev)
    //   show_warnings = 0 (Play) / 0 (Dev)
    // [Rendering] (Dev 模式下可选激活缓冲调整)
    //   allow_buffer_resize = 1

    let hunting_val = match mode {
        MigotoWorkMode::Play => "0",
        MigotoWorkMode::Dev => "2",
    };
    let marking_actions_val = match mode {
        MigotoWorkMode::Play => "clipboard",
        MigotoWorkMode::Dev => "clipboard hlsl asm regex",
    };

    set_ini_key_value(&mut lines, "Hunting", "hunting", hunting_val);
    set_ini_key_value(
        &mut lines,
        "Hunting",
        "marking_actions",
        marking_actions_val,
    );

    if mode == MigotoWorkMode::Play {
        set_ini_key_value(&mut lines, "Logging", "calls", "0");
        set_ini_key_value(&mut lines, "Logging", "show_warnings", "0");
    }

    let mut result = lines.join("\r\n");
    if !result.ends_with("\r\n") && !result.is_empty() {
        result.push_str("\r\n");
    }
    result
}

/// 同步更新 d3dx.ini 中的 [Loader] target 字段为当前配置的游戏可执行文件绝对路径
pub fn update_d3dx_ini_target(ini_path: &Path, target_exe: &Path) -> Result<()> {
    if !ini_path.is_file() {
        return Ok(());
    }
    let content = std::fs::read_to_string(ini_path)?;
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let win_path = target_exe.to_string_lossy().replace('/', "\\");
    set_ini_key_value(&mut lines, "Loader", "target", &win_path);
    let mut result = lines.join("\r\n");
    if !result.ends_with("\r\n") && !result.is_empty() {
        result.push_str("\r\n");
    }
    std::fs::write(ini_path, result)?;
    Ok(())
}

/// 辅助函数：在 INI 行列表中定位指定 section 下的 key 并替换其值；若未找到则安全追加
fn set_ini_key_value(
    lines: &mut Vec<String>,
    target_section: &str,
    target_key: &str,
    new_value: &str,
) {
    let mut current_section: Option<String> = None;
    let mut section_start_idx: Option<usize> = None;
    let mut key_found_idx: Option<usize> = None;

    let target_sec_lower = target_section.to_lowercase();
    let target_key_lower = target_key.to_lowercase();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let sec_name = trimmed[1..trimmed.len() - 1].trim().to_lowercase();
            if sec_name == target_sec_lower {
                section_start_idx = Some(i);
            }
            current_section = Some(sec_name);
            continue;
        }

        if current_section.as_deref() == Some(&target_sec_lower) {
            // 剥离注释检查 key
            let mut code_part = trimmed;
            if let Some((clean, _)) = code_part.split_once(';') {
                code_part = clean.trim();
            }
            if let Some((clean, _)) = code_part.split_once('#') {
                code_part = clean.trim();
            }
            if let Some((k, _)) = code_part.split_once('=') {
                if k.trim().to_lowercase() == target_key_lower {
                    key_found_idx = Some(i);
                    break;
                }
            }
        }
    }

    if let Some(k_idx) = key_found_idx {
        // 保留原行的行内注释（如有）
        let original_line = &lines[k_idx];
        let comment_suffix = if let Some(idx) = original_line.find(';') {
            format!(" {}", &original_line[idx..])
        } else if let Some(idx) = original_line.find('#') {
            format!(" {}", &original_line[idx..])
        } else {
            String::new()
        };

        lines[k_idx] = format!("{} = {}{}", target_key, new_value, comment_suffix);
    } else if let Some(s_idx) = section_start_idx {
        // section 存在但 key 不存在，插入到该 section 开头之后
        lines.insert(s_idx + 1, format!("{} = {}", target_key, new_value));
    } else {
        // section 不存在，在末尾创建
        if !lines.is_empty() && !lines.last().map(|s| s.is_empty()).unwrap_or(false) {
            lines.push(String::new());
        }
        lines.push(format!("[{}]", target_section));
        lines.push(format!("{} = {}", target_key, new_value));
    }
}

/// 将目标模式写入指定路径的 `d3dx.ini`
pub fn update_d3dx_ini_mode(ini_path: &Path, mode: MigotoWorkMode) -> Result<()> {
    if !ini_path.is_file() {
        return Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("d3dx.ini 文件不存在：{}", ini_path.display()),
        )));
    }

    let original = std::fs::read_to_string(ini_path)?;
    let updated = apply_work_mode(&original, mode);
    std::fs::write(ini_path, updated)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModKeyBinding {
    /// INI 中的 section 原始名称（如 KeySwapHead）
    pub section: String,
    /// 原始按键配置（如 VK_SHIFT VK_UP）
    pub key: String,
    /// 格式化后的按键（如 Shift + ↑）
    pub formatted_key: String,
    /// 回退按键（如有，如 VK_SHIFT VK_LEFT）
    pub back: Option<String>,
    pub formatted_back: Option<String>,
    /// 触发模式（如 cycle / toggle / hold）
    pub key_type: Option<String>,
    /// 控制的变量名（如 $swapvar）
    pub variable: Option<String>,
    /// 支持的档位数量（如 0,1,2 -> 3 档）
    pub steps: Option<usize>,
    /// 注释或说明
    pub comment: Option<String>,
}

/// 将 3Dmigoto INI 中的原始按键字符串格式化为人话（如 "VK_SHIFT VK_UP" -> "Shift + ↑"）
pub fn format_key_combination(raw: &str) -> String {
    let mut parts = Vec::new();
    for token in raw.split_whitespace() {
        let t = token.trim().to_lowercase();
        if t.is_empty() || t.starts_with("no_") {
            continue;
        }
        let mapped = match t.as_str() {
            "vk_shift" | "shift" => "Shift".to_string(),
            "vk_control" | "vk_ctrl" | "ctrl" | "control" => "Ctrl".to_string(),
            "vk_menu" | "vk_alt" | "alt" | "menu" => "Alt".to_string(),
            "vk_up" | "up" => "↑".to_string(),
            "vk_down" | "down" => "↓".to_string(),
            "vk_left" | "left" => "←".to_string(),
            "vk_right" | "right" => "→".to_string(),
            "vk_space" | "space" => "Space".to_string(),
            "vk_tab" | "tab" => "Tab".to_string(),
            "vk_escape" | "vk_esc" | "esc" | "escape" => "Esc".to_string(),
            "vk_return" | "vk_enter" | "enter" | "return" => "Enter".to_string(),
            "vk_oem_plus" => "+".to_string(),
            "vk_oem_minus" => "-".to_string(),
            "vk_oem_1" => ";".to_string(),
            "vk_oem_2" => "/".to_string(),
            "vk_oem_3" => "`".to_string(),
            "vk_oem_4" => "[".to_string(),
            "vk_oem_5" => "\\".to_string(),
            "vk_oem_6" => "]".to_string(),
            "vk_oem_7" => "'".to_string(),
            "vk_oem_comma" => ",".to_string(),
            "vk_oem_period" => ".".to_string(),
            "vk_numpad0" => "Num 0".to_string(),
            "vk_numpad1" => "Num 1".to_string(),
            "vk_numpad2" => "Num 2".to_string(),
            "vk_numpad3" => "Num 3".to_string(),
            "vk_numpad4" => "Num 4".to_string(),
            "vk_numpad5" => "Num 5".to_string(),
            "vk_numpad6" => "Num 6".to_string(),
            "vk_numpad7" => "Num 7".to_string(),
            "vk_numpad8" => "Num 8".to_string(),
            "vk_numpad9" => "Num 9".to_string(),
            s if s.starts_with("vk_") => s[3..].to_uppercase(),
            _ => token.to_uppercase(),
        };
        parts.push(mapped);
    }
    if parts.is_empty() {
        raw.trim().to_string()
    } else {
        parts.join(" + ")
    }
}

/// 扫描指定 Mod 目录下的所有有效 `.ini` 文件，提取 `[Key...]` 按键绑定
pub fn scan_mod_keys(mod_dir: &Path) -> Vec<ModKeyBinding> {
    let mut bindings = Vec::new();
    if !mod_dir.is_dir() {
        return bindings;
    }

    fn collect_inis(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        if depth > 3 {
            return;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let fname = p
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if fname.ends_with(".ini")
                        && !fname.starts_with("disabled")
                        && !fname.starts_with('.')
                    {
                        out.push(p);
                    }
                } else if p.is_dir() {
                    let dname = p
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if !dname.starts_with("disabled") && !dname.starts_with('.') {
                        collect_inis(&p, depth + 1, out);
                    }
                }
            }
        }
    }

    let mut ini_files = Vec::new();
    collect_inis(mod_dir, 0, &mut ini_files);

    for ini_path in ini_files {
        if let Ok(content) = std::fs::read_to_string(&ini_path) {
            let mut current_sec: Option<String> = None;
            let mut current_key: Option<String> = None;
            let mut current_back: Option<String> = None;
            let mut current_type: Option<String> = None;
            let mut current_var: Option<String> = None;
            let mut current_steps: Option<usize> = None;
            let mut current_comment: Option<String> = None;
            let mut last_comment: Option<String> = None;

            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    last_comment = None;
                    continue;
                }
                if trimmed.starts_with(';') || trimmed.starts_with('#') {
                    last_comment = Some(
                        trimmed
                            .trim_start_matches(|c| c == ';' || c == '#' || c == ' ')
                            .trim()
                            .to_string(),
                    );
                    continue;
                }

                if trimmed.starts_with('[') && trimmed.ends_with(']') {
                    // 结算上一个 section
                    if let (Some(sec), Some(key)) = (current_sec.take(), current_key.take()) {
                        bindings.push(ModKeyBinding {
                            section: sec,
                            formatted_key: format_key_combination(&key),
                            key,
                            formatted_back: current_back.as_deref().map(format_key_combination),
                            back: current_back.take(),
                            key_type: current_type.take(),
                            variable: current_var.take(),
                            steps: current_steps.take(),
                            comment: current_comment.take(),
                        });
                    }

                    let sec_raw = trimmed[1..trimmed.len() - 1].trim();
                    if sec_raw.to_lowercase().starts_with("key") {
                        current_sec = Some(sec_raw.to_string());
                        current_key = None;
                        current_back = None;
                        current_type = None;
                        current_var = None;
                        current_steps = None;
                        current_comment = last_comment.take();
                    } else {
                        current_sec = None;
                        last_comment = None;
                    }
                    continue;
                }

                if current_sec.is_some() {
                    if let Some((k, v)) = trimmed.split_once('=') {
                        let k_clean = k.trim().to_lowercase();
                        let mut v_clean = v.trim();
                        if let Some((val, inline_comm)) = v_clean.split_once(';') {
                            v_clean = val.trim();
                            if current_comment.is_none() && !inline_comm.trim().is_empty() {
                                current_comment = Some(inline_comm.trim().to_string());
                            }
                        }
                        match k_clean.as_str() {
                            "key" => current_key = Some(v_clean.to_string()),
                            "back" => current_back = Some(v_clean.to_string()),
                            "type" => current_type = Some(v_clean.to_string()),
                            var if var.starts_with('$') => {
                                current_var = Some(k.trim().to_string());
                                let step_count = v_clean.split(',').count();
                                if step_count > 1 {
                                    current_steps = Some(step_count);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            // 结算文件末尾的 section
            if let (Some(sec), Some(key)) = (current_sec.take(), current_key.take()) {
                bindings.push(ModKeyBinding {
                    section: sec,
                    formatted_key: format_key_combination(&key),
                    key,
                    formatted_back: current_back.as_deref().map(format_key_combination),
                    back: current_back.take(),
                    key_type: current_type.take(),
                    variable: current_var.take(),
                    steps: current_steps.take(),
                    comment: current_comment.take(),
                });
            }
        }
    }

    bindings
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModHashEntry {
    pub section: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictModInfo {
    pub id: i64,
    pub character: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModConflict {
    pub hash: String,
    pub section: String,
    pub conflicting_mods: Vec<ConflictModInfo>,
}

/// 扫描指定 Mod 目录下的所有有效 `.ini` 文件，提取所有 `hash = ...` 覆盖项
pub fn scan_mod_hashes(mod_dir: &Path) -> Vec<ModHashEntry> {
    let mut hashes = Vec::new();
    if !mod_dir.is_dir() {
        return hashes;
    }

    fn collect_inis(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        if depth > 3 {
            return;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let fname = p
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if fname.ends_with(".ini")
                        && !fname.starts_with("disabled")
                        && !fname.starts_with('.')
                    {
                        out.push(p);
                    }
                } else if p.is_dir() {
                    let dname = p
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if !dname.starts_with("disabled") && !dname.starts_with('.') {
                        collect_inis(&p, depth + 1, out);
                    }
                }
            }
        }
    }

    let mut ini_files = Vec::new();
    collect_inis(mod_dir, 0, &mut ini_files);

    for ini_path in ini_files {
        if let Ok(content) = std::fs::read_to_string(&ini_path) {
            let mut current_sec = String::new();

            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
                    continue;
                }

                if trimmed.starts_with('[') && trimmed.ends_with(']') {
                    current_sec = trimmed[1..trimmed.len() - 1].trim().to_string();
                    continue;
                }

                if let Some((k, v)) = trimmed.split_once('=') {
                    if k.trim().eq_ignore_ascii_case("hash") {
                        let mut val = v.trim();
                        if let Some((clean, _)) = val.split_once(';') {
                            val = clean.trim();
                        }
                        if let Some((clean, _)) = val.split_once('#') {
                            val = clean.trim();
                        }
                        if !val.is_empty() {
                            hashes.push(ModHashEntry {
                                section: current_sec.clone(),
                                hash: val.to_lowercase(),
                            });
                        }
                    }
                }
            }
        }
    }

    hashes
}

/// 分析当前仓库中所有已启用的 Mod，检测是否存在 Hash 碰撞冲突
pub fn detect_conflicts(lib: &crate::library::Library) -> crate::error::Result<Vec<ModConflict>> {
    let mods = lib.db.list_mods()?;
    let enabled_mods: Vec<_> = mods.into_iter().filter(|m| m.enabled).collect();
    if enabled_mods.len() <= 1 {
        return Ok(Vec::new());
    }

    let mut hash_map: HashMap<String, (String, Vec<ConflictModInfo>)> = HashMap::new();

    for m in &enabled_mods {
        let mod_dir = lib.layout.mod_dir(&m.character, &m.name);
        let entries = scan_mod_hashes(&mod_dir);
        let info = ConflictModInfo {
            id: m.id,
            character: m.character.clone(),
            name: m.name.clone(),
        };

        for entry in entries {
            let item = hash_map
                .entry(entry.hash)
                .or_insert_with(|| (entry.section, Vec::new()));
            if !item.1.iter().any(|x| x.id == info.id) {
                item.1.push(info.clone());
            }
        }
    }

    let mut conflicts = Vec::new();
    for (hash, (section, mod_list)) in hash_map {
        if mod_list.len() > 1 {
            conflicts.push(ModConflict {
                hash,
                section,
                conflicting_mods: mod_list,
            });
        }
    }

    conflicts.sort_by(|a, b| a.hash.cmp(&b.hash));
    Ok(conflicts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_standard_d3dx_ini() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        // 放置 mock loader exe
        fs::write(root.join("3DMigoto Loader.exe"), b"mock exe").unwrap();
        fs::create_dir_all(root.join("Mods")).unwrap();

        let ini_content = r#"
; 3Dmigoto Config
[Loader]
target = D:\Games\Star Rail\Game\StarRail.exe
loader = 3DMigoto Loader.exe
module = d3d11.dll
require_admin = true

[Include]
include = Core\SRMI\main.ini
include_recursive = Mods
exclude_recursive = DISABLED*
"#;

        let info = parse_d3dx_ini(ini_content, root);
        assert_eq!(
            info.game_exe,
            Some(PathBuf::from(r"D:\Games\Star Rail\Game\StarRail.exe"))
        );
        assert_eq!(info.loader_exe, Some(root.join("3DMigoto Loader.exe")));
        assert_eq!(info.mods_dir, Some(root.join("Mods")));
        assert_eq!(info.ini_path, root.join("d3dx.ini"));
    }

    #[test]
    fn parses_with_whitespace_and_comments() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        fs::write(root.join("3DMigotoLoader.exe"), b"mock").unwrap();

        let ini_content = r#"
[loader]  ; loader section
  target = "C:\StarRail\StarRail.exe" ; inline comment
  loader = 3DMigotoLoader.exe

[INCLUDE]
  include_recursive = "CustomMods"
"#;

        let info = parse_d3dx_ini(ini_content, root);
        assert_eq!(
            info.game_exe,
            Some(PathBuf::from(r"C:\StarRail\StarRail.exe"))
        );
        assert_eq!(info.loader_exe, Some(root.join("3DMigotoLoader.exe")));
        assert_eq!(info.mods_dir, Some(root.join("CustomMods")));
    }

    #[test]
    fn parses_srmi_real_sample() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        fs::write(root.join("3DMigotoLoader.exe"), b"mock").unwrap();
        fs::create_dir_all(root.join("Mods")).unwrap();

        let ini_content = r#"
[Loader]
target = D:\Star Rail\Game\StarRail.exe
loader = 3DMigotoLoader.exe
module = d3d11.dll
require_admin = true

[Include]
include = Core\SRMI\main.ini
include_recursive = Mods
exclude_recursive = DISABLED*
"#;

        let info = parse_d3dx_ini(ini_content, root);
        assert_eq!(
            info.game_exe,
            Some(PathBuf::from(r"D:\Star Rail\Game\StarRail.exe"))
        );
        assert_eq!(info.loader_exe, Some(root.join("3DMigotoLoader.exe")));
        assert_eq!(info.mods_dir, Some(root.join("Mods")));
    }

    #[test]
    fn inspect_migoto_dir_locates_from_mods_subfolder() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let mods = root.join("Mods");
        fs::create_dir_all(&mods).unwrap();
        fs::write(
            root.join("d3dx.ini"),
            "[Loader]\ntarget = C:\\Games\\Game.exe\n",
        )
        .unwrap();

        // 用户传了 Mods 目录
        let info = inspect_migoto_dir(&mods).unwrap();
        assert_eq!(info.game_exe, Some(PathBuf::from(r"C:\Games\Game.exe")));
        assert_eq!(info.mods_dir.unwrap().file_name().unwrap(), "Mods");
    }

    #[test]
    fn inspect_migoto_dir_errors_when_no_ini() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let err = inspect_migoto_dir(root).unwrap_err();
        assert!(err.to_string().contains("d3dx.ini"));
    }

    #[test]
    fn parses_mod_key_bindings_and_formatting() {
        let temp = tempfile::tempdir().unwrap();
        let mod_dir = temp.path();
        let ini = r#"
; 换装按键
[KeySwapCostume]
condition = $active == 1
key = VK_SHIFT VK_UP
back = VK_SHIFT VK_DOWN
type = cycle
$swapcostume = 0,1,2,3

; 武器切换
[KeyWeapon]
key = f ; 单键切换
$weapon = 0,1
"#;
        std::fs::write(mod_dir.join("test.ini"), ini).unwrap();

        let keys = scan_mod_keys(mod_dir);
        assert_eq!(keys.len(), 2);

        let k1 = &keys[0];
        assert_eq!(k1.section, "KeySwapCostume");
        assert_eq!(k1.formatted_key, "Shift + ↑");
        assert_eq!(k1.formatted_back.as_deref(), Some("Shift + ↓"));
        assert_eq!(k1.steps, Some(4));
        assert_eq!(k1.variable.as_deref(), Some("$swapcostume"));
        assert_eq!(k1.comment.as_deref(), Some("换装按键"));

        let k2 = &keys[1];
        assert_eq!(k2.section, "KeyWeapon");
        assert_eq!(k2.formatted_key, "F");
        assert_eq!(k2.comment.as_deref(), Some("武器切换"));
    }

    #[test]
    fn detects_mod_hash_conflicts_between_enabled_mods() {
        let temp = tempfile::tempdir().unwrap();
        let lib = crate::library::Library::init(temp.path()).unwrap();

        // 创建两个 Mod，分别包含相同 hash
        let mod1_dir = lib.layout.mod_dir("Acheron", "ModA");
        let mod2_dir = lib.layout.mod_dir("Acheron", "ModB");
        std::fs::create_dir_all(&mod1_dir).unwrap();
        std::fs::create_dir_all(&mod2_dir).unwrap();

        std::fs::write(
            mod1_dir.join("a.ini"),
            "[TextureOverrideBody]\nhash = 9de39691\n",
        )
        .unwrap();
        std::fs::write(
            mod2_dir.join("b.ini"),
            "[TextureOverrideDress]\nhash = 9de39691\n",
        )
        .unwrap();

        let id1 = lib
            .db
            .upsert_mod("Acheron", "ModA", "mods/Acheron/ModA")
            .unwrap();
        let id2 = lib
            .db
            .upsert_mod("Acheron", "ModB", "mods/Acheron/ModB")
            .unwrap();

        // 未启用时冲突为 0
        let conflicts = detect_conflicts(&lib).unwrap();
        assert_eq!(conflicts.len(), 0);

        // 仅启用 1 个时冲突为 0
        lib.db.set_enabled(id1, true).unwrap();
        let conflicts = detect_conflicts(&lib).unwrap();
        assert_eq!(conflicts.len(), 0);

        // 同时启用两个时，精准检测出冲突
        lib.db.set_enabled(id2, true).unwrap();
        let conflicts = detect_conflicts(&lib).unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].hash, "9de39691");
        assert_eq!(conflicts[0].conflicting_mods.len(), 2);
    }

    #[test]
    fn inspects_and_applies_work_modes() {
        let sample_ini = r#"; 3Dmigoto d3dx.ini template
[Loader]
target = StarRail.exe

[Hunting]
hunting = 0 ; 默认关闭
marking_actions = clipboard

[Logging]
calls = 0
show_warnings = 0
"#;
        // 初始状态为 Play 模式
        assert_eq!(inspect_work_mode(sample_ini), MigotoWorkMode::Play);

        // 切换为 Dev 模式
        let dev_ini = apply_work_mode(sample_ini, MigotoWorkMode::Dev);
        assert_eq!(inspect_work_mode(&dev_ini), MigotoWorkMode::Dev);
        assert!(dev_ini.contains("hunting = 2"));
        assert!(dev_ini.contains("marking_actions = clipboard hlsl asm regex"));

        // 切换回 Play 模式
        let play_ini = apply_work_mode(&dev_ini, MigotoWorkMode::Play);
        assert_eq!(inspect_work_mode(&play_ini), MigotoWorkMode::Play);
        assert!(play_ini.contains("hunting = 0"));
        assert!(play_ini.contains("marking_actions = clipboard"));
    }
}
