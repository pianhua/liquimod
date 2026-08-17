pub mod hsr;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CharacterInfo {
    pub internal_name: String,
    pub display_name: String,
    pub image: String,
}

pub trait Game {
    fn id(&self) -> &'static str;
    fn characters(&self) -> &[CharacterInfo];
}
