use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
}

#[derive(Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

impl Config {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}
