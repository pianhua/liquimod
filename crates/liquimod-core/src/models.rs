#[derive(Debug, Clone, PartialEq)]
pub struct ModEntry {
    pub id: i64,
    pub character: String,
    pub name: String,
    pub rel_path: String,
    pub enabled: bool,
    pub installed_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
}
