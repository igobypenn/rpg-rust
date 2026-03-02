use std::collections::HashMap;

mod utils;

pub struct AppConfig {
    name: String,
    version: String,
    settings: HashMap<String, String>,
}

impl AppConfig {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            settings: HashMap::new(),
        }
    }

    pub fn with_version(mut self, version: &str) -> Self {
        self.version = version.to_string();
        self
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.settings.insert(key.to_string(), value.to_string());
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.settings.get(key)
    }
}

pub fn create_config(name: &str) -> AppConfig {
    AppConfig::new(name)
}

pub fn merge_configs(base: AppConfig, override_cfg: AppConfig) -> AppConfig {
    let mut merged = base;
    for (k, v) in override_cfg.settings {
        merged.settings.insert(k, v);
    }
    merged
}
