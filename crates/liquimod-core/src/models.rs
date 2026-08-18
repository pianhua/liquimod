#[derive(Debug, Clone, PartialEq)]
pub struct ModEntry {
    pub id: i64,
    pub character: String,
    pub name: String,
    pub rel_path: String,
    pub enabled: bool,
    pub installed_at: i64,
    /// 目录总字节数；-1 = 未统计
    pub size_bytes: i64,
    /// 文件数；-1 = 未统计
    pub file_count: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
}
