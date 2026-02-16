use std::path::Path;
use std::str::FromStr;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::db::queries;
use crate::db::Database;
use crate::embeddings::error::EmbeddingError;

pub const EMBEDDING_CONFIG_SETTING_KEY: &str = "embedding_config";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EmbeddingProviderId {
    Gemini,
    Ollama,
    Transformersjs,
    RustHf,
}

impl EmbeddingProviderId {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Gemini => "gemini",
            Self::Ollama => "ollama",
            Self::Transformersjs => "transformersjs",
            Self::RustHf => "rust-hf",
        }
    }

    pub const fn all() -> &'static [EmbeddingProviderId] {
        &[
            EmbeddingProviderId::Gemini,
            EmbeddingProviderId::Ollama,
            EmbeddingProviderId::Transformersjs,
            EmbeddingProviderId::RustHf,
        ]
    }
}

impl std::fmt::Display for EmbeddingProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for EmbeddingProviderId {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "gemini" => Ok(Self::Gemini),
            "ollama" => Ok(Self::Ollama),
            "transformersjs" | "transformers-js" | "transformers_js" => Ok(Self::Transformersjs),
            "rust-hf" | "rust_hf" | "rusthf" => Ok(Self::RustHf),
            _ => Err(format!("unsupported embedding provider: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RustHfRuntime {
    Onnx,
    Candle,
}

impl Default for RustHfRuntime {
    fn default() -> Self {
        Self::Onnx
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiEmbeddingConfig {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_gemini_embedding_model")]
    pub model: String,
    #[serde(default = "default_remote_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaEmbeddingConfig {
    #[serde(default = "default_ollama_base_url")]
    pub base_url: String,
    #[serde(default = "default_ollama_embedding_model")]
    pub model: String,
    #[serde(default = "default_remote_timeout_ms")]
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformersJsEmbeddingConfig {
    #[serde(default = "default_transformersjs_model")]
    pub model: String,
    #[serde(default = "default_transformersjs_device")]
    pub device: String,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub cache_dir: Option<String>,
    #[serde(default = "default_local_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_transformersjs_bridge_command")]
    pub bridge_command: String,
    #[serde(default)]
    pub bridge_script: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustHfEmbeddingConfig {
    #[serde(default = "default_rust_hf_model_id")]
    pub model_id: String,
    #[serde(default)]
    pub model_path: Option<String>,
    #[serde(default)]
    pub cache_dir: Option<String>,
    #[serde(default)]
    pub runtime: RustHfRuntime,
    #[serde(default)]
    pub threads: Option<usize>,
    #[serde(default = "default_local_timeout_ms")]
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub provider: EmbeddingProviderId,
    #[serde(default)]
    pub normalize_l2: bool,
    #[serde(default)]
    pub gemini: GeminiEmbeddingConfig,
    #[serde(default)]
    pub ollama: OllamaEmbeddingConfig,
    #[serde(default)]
    pub transformersjs: TransformersJsEmbeddingConfig,
    #[serde(default)]
    pub rust_hf: RustHfEmbeddingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiEmbeddingConfigView {
    pub api_key_configured: bool,
    pub model: String,
    pub timeout_ms: u64,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfigView {
    pub enabled: bool,
    pub provider: EmbeddingProviderId,
    pub normalize_l2: bool,
    pub gemini: GeminiEmbeddingConfigView,
    pub ollama: OllamaEmbeddingConfig,
    pub transformersjs: TransformersJsEmbeddingConfig,
    pub rust_hf: RustHfEmbeddingConfig,
}

impl Default for GeminiEmbeddingConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            model: default_gemini_embedding_model(),
            timeout_ms: default_remote_timeout_ms(),
            base_url: None,
        }
    }
}

impl Default for OllamaEmbeddingConfig {
    fn default() -> Self {
        Self {
            base_url: default_ollama_base_url(),
            model: default_ollama_embedding_model(),
            timeout_ms: default_remote_timeout_ms(),
        }
    }
}

impl Default for TransformersJsEmbeddingConfig {
    fn default() -> Self {
        Self {
            model: default_transformersjs_model(),
            device: default_transformersjs_device(),
            backend: None,
            cache_dir: None,
            timeout_ms: default_local_timeout_ms(),
            bridge_command: default_transformersjs_bridge_command(),
            bridge_script: None,
        }
    }
}

impl Default for RustHfEmbeddingConfig {
    fn default() -> Self {
        Self {
            model_id: default_rust_hf_model_id(),
            model_path: None,
            cache_dir: None,
            runtime: RustHfRuntime::Onnx,
            threads: None,
            timeout_ms: default_local_timeout_ms(),
        }
    }
}

impl Default for EmbeddingProviderId {
    fn default() -> Self {
        Self::Ollama
    }
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: EmbeddingProviderId::default(),
            normalize_l2: false,
            gemini: GeminiEmbeddingConfig::default(),
            ollama: OllamaEmbeddingConfig::default(),
            transformersjs: TransformersJsEmbeddingConfig::default(),
            rust_hf: RustHfEmbeddingConfig::default(),
        }
    }
}

impl EmbeddingConfig {
    pub fn apply_env_overrides(&mut self) {
        if self
            .gemini
            .api_key
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
        {
            if let Ok(value) = std::env::var("GEMINI_API_KEY") {
                if !value.trim().is_empty() {
                    self.gemini.api_key = Some(value);
                }
            } else if let Ok(value) = std::env::var("GOOGLE_API_KEY") {
                if !value.trim().is_empty() {
                    self.gemini.api_key = Some(value);
                }
            }
        }

        if let Ok(provider_override) = std::env::var("ORCHESTRIX_EMBEDDING_PROVIDER") {
            if let Ok(provider) = EmbeddingProviderId::from_str(&provider_override) {
                self.provider = provider;
            }
        }
    }

    pub fn effective_gemini_api_key(&self) -> Option<String> {
        let from_config = self.gemini.api_key.as_deref().unwrap_or("").trim();
        if !from_config.is_empty() {
            return Some(from_config.to_string());
        }
        if let Ok(value) = std::env::var("GEMINI_API_KEY") {
            if !value.trim().is_empty() {
                return Some(value.trim().to_string());
            }
        }
        if let Ok(value) = std::env::var("GOOGLE_API_KEY") {
            if !value.trim().is_empty() {
                return Some(value.trim().to_string());
            }
        }
        None
    }

    pub fn validate_selected_provider(&self) -> Result<(), EmbeddingError> {
        match self.provider {
            EmbeddingProviderId::Gemini => {
                if self.effective_gemini_api_key().is_none() {
                    return Err(EmbeddingError::Config(
                        "gemini embedding provider requires apiKey (set embedding.gemini.apiKey or GEMINI_API_KEY)"
                            .to_string(),
                    ));
                }
                if self.gemini.model.trim().is_empty() {
                    return Err(EmbeddingError::Config(
                        "gemini model cannot be empty".to_string(),
                    ));
                }
                if self.gemini.timeout_ms == 0 {
                    return Err(EmbeddingError::Config(
                        "gemini timeout must be greater than 0".to_string(),
                    ));
                }
            }
            EmbeddingProviderId::Ollama => {
                if self.ollama.model.trim().is_empty() {
                    return Err(EmbeddingError::Config(
                        "ollama model cannot be empty".to_string(),
                    ));
                }
                if self.ollama.timeout_ms == 0 {
                    return Err(EmbeddingError::Config(
                        "ollama timeout must be greater than 0".to_string(),
                    ));
                }
                reqwest::Url::parse(self.ollama.base_url.trim()).map_err(|error| {
                    EmbeddingError::Config(format!(
                        "invalid ollama baseUrl '{}': {error}",
                        self.ollama.base_url
                    ))
                })?;
            }
            EmbeddingProviderId::Transformersjs => {
                if self.transformersjs.model.trim().is_empty() {
                    return Err(EmbeddingError::Config(
                        "transformersjs model cannot be empty".to_string(),
                    ));
                }
                if self.transformersjs.timeout_ms == 0 {
                    return Err(EmbeddingError::Config(
                        "transformersjs timeout must be greater than 0".to_string(),
                    ));
                }
                if self.transformersjs.bridge_command.trim().is_empty() {
                    return Err(EmbeddingError::Config(
                        "transformersjs bridge command cannot be empty".to_string(),
                    ));
                }
            }
            EmbeddingProviderId::RustHf => {
                if self.rust_hf.timeout_ms == 0 {
                    return Err(EmbeddingError::Config(
                        "rust-hf timeout must be greater than 0".to_string(),
                    ));
                }
                if self.rust_hf.model_id.trim().is_empty() && self.rust_hf.model_path.is_none() {
                    return Err(EmbeddingError::Config(
                        "rust-hf requires modelId or modelPath".to_string(),
                    ));
                }
                if let Some(threads) = self.rust_hf.threads {
                    if threads == 0 {
                        return Err(EmbeddingError::Config(
                            "rust-hf threads must be greater than 0".to_string(),
                        ));
                    }
                }
                if let Some(model_path) = self.rust_hf.model_path.as_deref() {
                    if !Path::new(model_path).exists() {
                        return Err(EmbeddingError::Config(format!(
                            "rust-hf modelPath does not exist: {model_path}"
                        )));
                    }
                } else if !self.rust_hf.model_id.trim().is_empty()
                    && self
                        .rust_hf
                        .model_id
                        .parse::<fastembed::EmbeddingModel>()
                        .is_err()
                {
                    return Err(EmbeddingError::Config(format!(
                        "unknown rust-hf modelId '{}'. Use a supported fastembed model code or provide modelPath",
                        self.rust_hf.model_id
                    )));
                }
            }
        }
        Ok(())
    }

    pub fn to_view(&self) -> EmbeddingConfigView {
        EmbeddingConfigView {
            enabled: self.enabled,
            provider: self.provider,
            normalize_l2: self.normalize_l2,
            gemini: GeminiEmbeddingConfigView {
                api_key_configured: self.effective_gemini_api_key().is_some(),
                model: self.gemini.model.clone(),
                timeout_ms: self.gemini.timeout_ms,
                base_url: self.gemini.base_url.clone(),
            },
            ollama: self.ollama.clone(),
            transformersjs: self.transformersjs.clone(),
            rust_hf: self.rust_hf.clone(),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.enabled && self.validate_selected_provider().is_ok()
    }
}

pub fn load_embedding_config(db: &Database) -> Result<EmbeddingConfig, EmbeddingError> {
    let raw = queries::get_setting(db, EMBEDDING_CONFIG_SETTING_KEY)
        .map_err(|error| EmbeddingError::Runtime(error.to_string()))?;

    let mut config = if let Some(raw) = raw {
        serde_json::from_str::<EmbeddingConfig>(&raw).map_err(|error| {
            EmbeddingError::Config(format!(
                "invalid embedding configuration in settings: {error}"
            ))
        })?
    } else {
        EmbeddingConfig::default()
    };

    config.apply_env_overrides();
    Ok(config)
}

pub fn save_embedding_config(
    db: &Database,
    config: &EmbeddingConfig,
) -> Result<(), EmbeddingError> {
    let value = serde_json::to_string(config)
        .map_err(|error| EmbeddingError::Runtime(format!("failed to serialize config: {error}")))?;

    queries::upsert_setting(
        db,
        EMBEDDING_CONFIG_SETTING_KEY,
        &value,
        &Utc::now().to_rfc3339(),
    )
    .map_err(|error| EmbeddingError::Runtime(error.to_string()))
}

fn default_gemini_embedding_model() -> String {
    "gemini-embedding-001".to_string()
}

fn default_ollama_embedding_model() -> String {
    "nomic-embed-text".to_string()
}

fn default_ollama_base_url() -> String {
    "http://127.0.0.1:11434".to_string()
}

fn default_transformersjs_model() -> String {
    "Xenova/all-MiniLM-L6-v2".to_string()
}

fn default_transformersjs_device() -> String {
    "cpu".to_string()
}

fn default_transformersjs_bridge_command() -> String {
    "bun".to_string()
}

fn default_rust_hf_model_id() -> String {
    "Qdrant/all-MiniLM-L6-v2-onnx".to_string()
}

fn default_remote_timeout_ms() -> u64 {
    30_000
}

fn default_local_timeout_ms() -> u64 {
    120_000
}
