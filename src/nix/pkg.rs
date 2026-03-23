use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Package {
    pub name: String,
    pub description: String,
}
