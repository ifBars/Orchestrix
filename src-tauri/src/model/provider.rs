//! Provider ID type and parsing utilities.

use std::fmt;
use std::str::FromStr;

/// Supported model providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderId {
    MiniMax,
    Kimi,
    Zhipu,
    Modal,
}

impl ProviderId {
    pub const fn as_str(&self) -> &'static str {
        match self {
            ProviderId::MiniMax => "minimax",
            ProviderId::Kimi => "kimi",
            ProviderId::Zhipu => "zhipu",
            ProviderId::Modal => "modal",
        }
    }

    pub const fn all() -> &'static [ProviderId] {
        &[
            ProviderId::MiniMax,
            ProviderId::Kimi,
            ProviderId::Zhipu,
            ProviderId::Modal,
        ]
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ProviderId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "minimax" => Ok(ProviderId::MiniMax),
            "kimi" => Ok(ProviderId::Kimi),
            "zhipu" | "glm" => Ok(ProviderId::Zhipu),
            "modal" => Ok(ProviderId::Modal),
            _ => Err(format!("unknown provider: {}", s)),
        }
    }
}

impl serde::Serialize for ProviderId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for ProviderId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ProviderId::from_str(&s).map_err(serde::de::Error::custom)
    }
}

/// Parse a model override string in `<provider>/<model>` format.
/// Returns (provider_override, model_name) where provider_override is None if not specified.
pub fn parse_model_override(value: &str) -> (Option<ProviderId>, String) {
    let trimmed = value.trim();
    if let Some((provider_str, model)) = trimmed.split_once('/') {
        let provider = ProviderId::from_str(provider_str.trim()).ok();
        let model = model.trim().to_string();
        if !model.is_empty() {
            return (provider, model);
        }
    }
    (None, trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_id_parsing() {
        assert_eq!(
            ProviderId::from_str("minimax").unwrap(),
            ProviderId::MiniMax
        );
        assert_eq!(ProviderId::from_str("kimi").unwrap(), ProviderId::Kimi);
        assert_eq!(ProviderId::from_str("zhipu").unwrap(), ProviderId::Zhipu);
        assert_eq!(ProviderId::from_str("glm").unwrap(), ProviderId::Zhipu);
        assert_eq!(ProviderId::from_str("modal").unwrap(), ProviderId::Modal);
        assert_eq!(
            ProviderId::from_str("MINIMAX").unwrap(),
            ProviderId::MiniMax
        );
        assert!(ProviderId::from_str("unknown").is_err());
    }

    #[test]
    fn test_parse_model_override() {
        let (provider, model) = parse_model_override("kimi/kimi-k2.5");
        assert_eq!(provider, Some(ProviderId::Kimi));
        assert_eq!(model, "kimi-k2.5");

        let (provider, model) = parse_model_override("minimax/MiniMax-M2.1");
        assert_eq!(provider, Some(ProviderId::MiniMax));
        assert_eq!(model, "MiniMax-M2.1");

        let (provider, model) = parse_model_override("MiniMax-M2.1");
        assert_eq!(provider, None);
        assert_eq!(model, "MiniMax-M2.1");
    }
}
