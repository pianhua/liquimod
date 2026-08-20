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
    /// 所属自定义分类；None = 角色视图（默认）
    pub category_id: Option<i64>,
    /// Mod 备忘与备注
    pub note: Option<String>,
    /// 自定义封面图片相对路径；None = 自动探测
    pub cover_image: Option<String>,
    /// 是否标为喜爱（置顶）
    pub is_favorite: bool,
    /// 自定义排序序号（拖拽排序）
    pub sort_order: i64,
    /// 当前选择的变体目录；无变体时为 None
    pub active_variant: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub ord: i64,
    /// 固定分类的内部标识（lightcone/portrait/scene/npc/other）；None = 用户自定义分类
    pub kind: Option<String>,
    pub mod_count: i64,
}
