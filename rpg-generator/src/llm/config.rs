//! LLM configuration types.

#[derive(Clone, Debug)]
pub struct LlmConfig {
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: usize,
    pub max_concurrent: usize,
}

impl LlmConfig {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: None,
            model: "gpt-4o".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
            max_concurrent: 5,
        }
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = temp.clamp(0.0, 2.0);
        self
    }

    pub fn with_max_tokens(mut self, max: usize) -> Self {
        self.max_tokens = max;
        self
    }

    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max;
        self
    }

    pub fn from_env() -> crate::Result<Self> {
        let api_key =
            std::env::var("OPENAI_API_KEY").map_err(|_| crate::GeneratorError::Environment {
                tool: "OPENAI_API_KEY".to_string(),
                install_hint: "Set the OPENAI_API_KEY environment variable".to_string(),
            })?;

        let mut config = Self::new(api_key);

        if let Ok(base_url) = std::env::var("OPENAI_BASE_URL") {
            config.base_url = Some(base_url);
        }

        if let Ok(model) = std::env::var("OPENAI_MODEL") {
            config.model = model;
        }

        Ok(config)
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: None,
            model: "gpt-4o".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
            max_concurrent: 5,
        }
    }
}
