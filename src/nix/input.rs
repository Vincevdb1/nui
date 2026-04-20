#[derive(Debug, Clone, Default)]
pub struct Input {
    pub name: String,
    pub url: String,
    pub branch: Option<String>,
    pub rev: Option<String>,
}
