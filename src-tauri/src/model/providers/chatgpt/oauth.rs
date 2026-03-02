//! ChatGPT OAuth authentication for ChatGPT Plus/Pro subscriptions
//! 
//! Implements PKCE OAuth flow to authenticate with OpenAI and obtain
//! access tokens for the ChatGPT Codex API.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::Rng;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

/// OpenAI OAuth client ID for Codex CLI (also used by OpenCode)
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const ISSUER: &str = "https://auth.openai.com";

/// PKCE challenge codes
#[derive(Debug, Clone)]
pub struct PkceCodes {
    pub verifier: String,
    pub challenge: String,
}

impl PkceCodes {
    /// Generate PKCE codes for OAuth flow
    pub fn generate() -> Self {
        let verifier = generate_random_string(128);
        let challenge = base64_url_encode(&sha256_hash(&verifier));
        Self { verifier, challenge }
    }
}

/// OAuth token response from OpenAI
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TokenResponse {
    pub id_token: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: Option<u64>,
}

/// Parsed JWT claims from OpenAI tokens
#[derive(Debug, Clone, serde::Deserialize)]
pub struct IdTokenClaims {
    pub chatgpt_account_id: Option<String>,
    pub organizations: Option<Vec<Organization>>,
    pub email: Option<String>,
    #[serde(rename = "https://api.openai.com/auth")]
    pub openai_auth: Option<OpenAIAuth>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Organization {
    pub id: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct OpenAIAuth {
    pub chatgpt_account_id: Option<String>,
}

/// Extract account ID from token claims
pub fn extract_account_id(claims: &IdTokenClaims) -> Option<String> {
    claims.chatgpt_account_id.clone()
        .or_else(|| claims.openai_auth.as_ref()?.chatgpt_account_id.clone())
        .or_else(|| Some(claims.organizations.as_ref()?.first()?.id.clone()))
}

/// Parse JWT claims from a token
pub fn parse_jwt_claims(token: &str) -> Option<IdTokenClaims> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    
    let payload = parts[1];
    let decoded = base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        payload
    ).ok()?;
    
    serde_json::from_slice(&decoded).ok()
}

/// Extract account ID from token response
pub fn extract_account_id_from_tokens(tokens: &TokenResponse) -> Option<String> {
    // Try ID token first
    if let Some(claims) = parse_jwt_claims(&tokens.id_token) {
        if let Some(account_id) = extract_account_id(&claims) {
            return Some(account_id);
        }
    }
    
    // Fall back to access token
    if let Some(claims) = parse_jwt_claims(&tokens.access_token) {
        return extract_account_id(&claims);
    }
    
    None
}

/// Build the authorization URL for OAuth flow
pub fn build_authorize_url(redirect_uri: &str, pkce: &PkceCodes, state: &str) -> String {
    let params = [
        ("response_type", "code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", redirect_uri),
        ("scope", "openid profile email offline_access"),
        ("code_challenge", &pkce.challenge),
        ("code_challenge_method", "S256"),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("state", state),
        ("originator", "orchestrix"),
    ];
    
    let query = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    
    format!("{}/oauth/authorize?{}", ISSUER, query)
}

/// Exchange authorization code for tokens
pub async fn exchange_code_for_tokens(
    code: &str,
    redirect_uri: &str,
    pkce: &PkceCodes,
) -> Result<TokenResponse, String> {
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", CLIENT_ID),
        ("code_verifier", &pkce.verifier),
    ];
    
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/oauth/token", ISSUER))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token request failed: {}", e))?;
    
    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Token exchange failed: {}", text));
    }
    
    response
        .json::<TokenResponse>()
        .await
        .map_err(|e| format!("Failed to parse token response: {}", e))
}

/// Refresh access token using refresh token
pub async fn refresh_access_token(refresh_token: &str) -> Result<TokenResponse, String> {
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", CLIENT_ID),
    ];
    
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/oauth/token", ISSUER))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Refresh request failed: {}", e))?;
    
    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Token refresh failed: {}", text));
    }
    
    response
        .json::<TokenResponse>()
        .await
        .map_err(|e| format!("Failed to parse refresh response: {}", e))
}

/// Generate a random state parameter
pub fn generate_state() -> String {
    let random_bytes: [u8; 32] = rand::thread_rng().gen();
    base64_url_encode(&random_bytes)
}

/// Calculate token expiration timestamp
pub fn calculate_expires_at(expires_in: u64) -> i64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    now + expires_in as i64
}

// Helper functions

fn generate_random_string(length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}

fn sha256_hash(input: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hasher.finalize().to_vec()
}

fn base64_url_encode(data: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkce_generation() {
        let pkce = PkceCodes::generate();
        assert!(!pkce.verifier.is_empty());
        assert!(!pkce.challenge.is_empty());
        assert_ne!(pkce.verifier, pkce.challenge);
    }

    #[test]
    fn test_build_authorize_url() {
        let pkce = PkceCodes {
            verifier: "test_verifier".to_string(),
            challenge: "test_challenge".to_string(),
        };
        let url = build_authorize_url("http://localhost:1455/callback", &pkce, "test_state");
        assert!(url.contains("auth.openai.com"));
        assert!(url.contains("test_challenge"));
        assert!(url.contains("test_state"));
        assert!(url.contains("orchestrix"));
    }

    #[test]
    fn test_generate_state() {
        let state1 = generate_state();
        let state2 = generate_state();
        assert_ne!(state1, state2);
        assert!(!state1.is_empty());
    }
}
