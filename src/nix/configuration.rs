#[derive(Debug, Clone, Default)]
pub struct Configuration {
    pub path: String,
    pub name: Option<String>,
    pub config_type: String,
}
