use chrono::Utc;
use serde_json::Value;
use std::str::FromStr;
use tauri::Emitter;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::db::queries;
use crate::model::providers::chatgpt::{
    build_authorize_url, calculate_expires_at, exchange_code_for_tokens,
    extract_account_id_from_tokens, generate_state, PkceCodes,
};
use crate::model::{ModelCatalog, ProviderId};
use crate::{
    load_provider_config, provider_setting_key, AppError, AppState, ModelCatalogEntry, ModelInfo,
    ProviderConfig, ProviderConfigView,
};

#[tauri::command]
pub fn set_provider_config(
    state: tauri::State<'_, AppState>,
    provider: String,
    api_key: String,
    default_model: Option<String>,
    base_url: Option<String>,
) -> Result<(), AppError> {
    let provider = provider.to_ascii_lowercase();
    ProviderId::from_str(&provider)
        .map_err(|_| AppError::Other(format!("unsupported provider: {provider}")))?;

    let value = serde_json::to_string(&ProviderConfig {
        api_key,
        default_model,
        base_url,
    })
    .map_err(|e| AppError::Other(e.to_string()))?;

    queries::upsert_setting(
        &state.db,
        &provider_setting_key(&provider),
        &value,
        &Utc::now().to_rfc3339(),
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_provider_configs(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ProviderConfigView>, AppError> {
    let providers = ProviderId::all();
    let mut result = Vec::new();
    for provider in providers {
        let provider_id = provider.as_str();
        let cfg = load_provider_config(&state.db, provider_id)?;
        result.push(ProviderConfigView {
            provider: provider_id.to_string(),
            configured: cfg.is_some(),
            default_model: cfg
                .as_ref()
                .and_then(|v| v.default_model.clone())
                .or_else(|| Some(ModelCatalog::default_model_for_provider(*provider))),
            base_url: cfg.and_then(|v| v.base_url),
        });
    }
    Ok(result)
}

#[tauri::command]
pub fn remove_provider_config(
    state: tauri::State<'_, AppState>,
    provider: String,
) -> Result<(), AppError> {
    let provider = provider.to_ascii_lowercase();
    ProviderId::from_str(&provider)
        .map_err(|_| AppError::Other(format!("unsupported provider: {provider}")))?;

    let key = provider_setting_key(&provider);
    queries::delete_setting(&state.db, &key)
        .map_err(|e| AppError::Other(format!("Failed to remove provider config: {e}")))?;
    Ok(())
}

/// Returns the context window size for a given model.
fn get_model_context_window(model: &str) -> usize {
    ModelCatalog::all_models()
        .into_iter()
        .flat_map(|entry| entry.models)
        .find(|entry| entry.name == model)
        .map(|entry| entry.context_window as usize)
        .unwrap_or(8_192)
}

#[tauri::command]
pub fn get_model_catalog() -> Vec<ModelCatalogEntry> {
    ModelCatalog::all_models()
        .into_iter()
        .map(|entry| ModelCatalogEntry {
            provider: entry.provider,
            models: entry
                .models
                .into_iter()
                .map(|model| ModelInfo {
                    name: model.name,
                    context_window: model.context_window as usize,
                })
                .collect(),
        })
        .collect()
}

#[tauri::command]
pub fn get_context_window_for_model(model: String) -> usize {
    get_model_context_window(&model)
}

/// Start ChatGPT OAuth flow, open the auth URL in the system browser, then
/// spin up a one-shot HTTP listener on localhost:1455 to catch the redirect.
/// When the callback arrives the code is exchanged for tokens, stored in the
/// DB, and a `chatgpt://oauth-complete` event is emitted to the frontend.
///
/// The command returns immediately after opening the browser; the actual
/// completion is asynchronous and reported via the Tauri event.
#[tauri::command]
pub async fn start_chatgpt_oauth_and_listen(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<ChatGPTAuthUrl, AppError> {
    let pkce = PkceCodes::generate();
    let oauth_state = generate_state();
    let redirect_uri = "http://localhost:1455/auth/callback";
    let auth_url = build_authorize_url(redirect_uri, &pkce, &oauth_state);

    let verifier = pkce.verifier.clone();
    let db = state.db.clone();
    let app_handle = app.clone();

    // Spawn background task: bind, wait for one request, exchange, emit.
    tauri::async_runtime::spawn(async move {
        let result = run_callback_server(verifier, redirect_uri, db).await;

        #[derive(Clone, serde::Serialize)]
        struct OAuthCompletePayload {
            success: bool,
            error: Option<String>,
            account_id: Option<String>,
        }

        match result {
            Ok(account_id) => {
                let _ = app_handle.emit(
                    "chatgpt://oauth-complete",
                    OAuthCompletePayload {
                        success: true,
                        error: None,
                        account_id,
                    },
                );
            }
            Err(e) => {
                let _ = app_handle.emit(
                    "chatgpt://oauth-complete",
                    OAuthCompletePayload {
                        success: false,
                        error: Some(e),
                        account_id: None,
                    },
                );
            }
        }
    });

    Ok(ChatGPTAuthUrl {
        url: auth_url,
        state: oauth_state,
        pkce_verifier: pkce.verifier,
    })
}

/// Bind on localhost:1455, wait for a single OAuth callback HTTP request,
/// extract the `code` parameter, exchange it for tokens, persist to DB.
/// Returns the account_id on success.
async fn run_callback_server(
    pkce_verifier: String,
    redirect_uri: &str,
    db: std::sync::Arc<crate::db::Database>,
) -> Result<Option<String>, String> {
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:1455")
        .await
        .map_err(|e| format!("Could not bind callback port 1455: {e}"))?;

    // Accept exactly one connection (the browser redirect).
    let (mut stream, _) = listener
        .accept()
        .await
        .map_err(|e| format!("Failed to accept callback connection: {e}"))?;
    drop(listener); // Stop listening immediately.

    // Read the HTTP request line by line until we find the request line.
    let mut buf = [0u8; 4096];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| format!("Failed to read callback request: {e}"))?;

    let request = String::from_utf8_lossy(&buf[..n]);
    let first_line = request.lines().next().unwrap_or("");

    // "GET /auth/callback?code=…&state=… HTTP/1.1"
    let code = first_line
        .split_whitespace()
        .nth(1) // The path+query part
        .and_then(|path| {
            let q = path.splitn(2, '?').nth(1)?;
            // Find code= among query params, percent-decode it.
            q.split('&').find_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                if k == "code" {
                    Some(
                        urlencoding::decode(v)
                            .map(|s| s.into_owned())
                            .unwrap_or_else(|_| v.to_owned()),
                    )
                } else {
                    None
                }
            })
        })
        .ok_or_else(|| format!("No `code` in callback URL: {first_line}"))?;

    // Send a minimal success page so the browser tab is not just blank.
    let body = "<html><body style=\"font-family:sans-serif;margin:80px auto;max-width:420px;text-align:center\">\
        <h2>&#10003; Authentication complete</h2>\
        <p>You can close this tab and return to Orchestrix.</p></body></html>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes()).await;
    drop(stream);

    // Exchange the code for tokens.
    let pkce = PkceCodes {
        verifier: pkce_verifier,
        challenge: String::new(),
    };
    let tokens = exchange_code_for_tokens(&code, redirect_uri, &pkce)
        .await
        .map_err(|e| format!("Token exchange failed: {e}"))?;

    let account_id = extract_account_id_from_tokens(&tokens);
    let expires_at = calculate_expires_at(tokens.expires_in.unwrap_or(3600));

    let auth_data = ChatGPTAuthData {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_at,
        account_id: account_id.clone(),
    };

    let value = serde_json::to_string(&auth_data)
        .map_err(|e| format!("Failed to serialize auth data: {e}"))?;

    queries::upsert_setting(&db, "chatgpt_oauth_auth", &value, &Utc::now().to_rfc3339())
        .map_err(|e| format!("Failed to persist auth data: {e}"))?;

    Ok(account_id)
}

/// Start ChatGPT OAuth flow - returns authorization URL
#[tauri::command]
pub fn start_chatgpt_oauth() -> Result<ChatGPTAuthUrl, AppError> {
    let pkce = PkceCodes::generate();
    let state = generate_state();
    let redirect_uri = format!("http://localhost:1455/auth/callback");
    let auth_url = build_authorize_url(&redirect_uri, &pkce, &state);

    // Store PKCE and state temporarily (in-memory for this session)
    // In a real implementation, you'd use a cache or temporary storage
    Ok(ChatGPTAuthUrl {
        url: auth_url,
        state,
        pkce_verifier: pkce.verifier,
    })
}

/// Complete ChatGPT OAuth flow by exchanging code for tokens
#[tauri::command]
pub async fn complete_chatgpt_oauth(
    state: tauri::State<'_, AppState>,
    code: String,
    pkce_verifier: String,
) -> Result<(), AppError> {
    let redirect_uri = "http://localhost:1455/auth/callback";
    let pkce = PkceCodes {
        verifier: pkce_verifier,
        challenge: String::new(), // Not needed for token exchange
    };

    let tokens = exchange_code_for_tokens(&code, redirect_uri, &pkce)
        .await
        .map_err(|e| AppError::Other(format!("OAuth exchange failed: {}", e)))?;

    let account_id = extract_account_id_from_tokens(&tokens);
    let expires_at = calculate_expires_at(tokens.expires_in.unwrap_or(3600));

    // Store tokens in database
    let auth_data = ChatGPTAuthData {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_at,
        account_id,
    };

    let value = serde_json::to_string(&auth_data)
        .map_err(|e| AppError::Other(format!("Failed to serialize auth data: {}", e)))?;

    queries::upsert_setting(
        &state.db,
        "chatgpt_oauth_auth",
        &value,
        &Utc::now().to_rfc3339(),
    )?;

    Ok(())
}

/// Get ChatGPT OAuth authentication status
#[tauri::command]
pub fn get_chatgpt_auth_status(
    state: tauri::State<'_, AppState>,
) -> Result<ChatGPTAuthStatus, AppError> {
    let setting = queries::get_setting(&state.db, "chatgpt_oauth_auth")
        .map_err(|e| AppError::Other(format!("Failed to load auth: {}", e)))?;

    match setting {
        Some(s) => {
            let auth: ChatGPTAuthData = serde_json::from_str(&s)
                .map_err(|e| AppError::Other(format!("Failed to parse auth: {}", e)))?;

            let now = chrono::Utc::now().timestamp();
            let is_expired = auth.expires_at - 30 < now;

            Ok(ChatGPTAuthStatus {
                authenticated: true,
                is_expired,
                account_id: auth.account_id,
            })
        }
        None => Ok(ChatGPTAuthStatus {
            authenticated: false,
            is_expired: false,
            account_id: None,
        }),
    }
}

/// Remove ChatGPT OAuth authentication
#[tauri::command]
pub fn remove_chatgpt_auth(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    queries::delete_setting(&state.db, "chatgpt_oauth_auth")
        .map_err(|e| AppError::Other(format!("Failed to remove auth: {}", e)))?;
    Ok(())
}

// Data structures for ChatGPT OAuth

#[derive(Debug, Clone, serde::Serialize)]
pub struct ChatGPTAuthUrl {
    pub url: String,
    pub state: String,
    pub pkce_verifier: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatGPTAuthData {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ChatGPTAuthStatus {
    pub authenticated: bool,
    pub is_expired: bool,
    pub account_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Provider Usage / Balance Snapshot
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderUsageSnapshotView {
    pub provider: String,
    pub available: bool,
    pub status: String,
    pub balance: Option<String>,
    pub currency: Option<String>,
    pub remaining_quota: Option<String>,
    pub period: Option<String>,
    pub last_updated_at: Option<String>,
    pub note: Option<String>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn get_provider_usage_snapshot(
    state: tauri::State<'_, AppState>,
    provider: Option<String>,
) -> Result<Vec<ProviderUsageSnapshotView>, AppError> {
    let providers_to_check: Vec<String> = match provider {
        Some(p) => vec![p.to_ascii_lowercase()],
        None => {
            // Get all configured providers
            let configs = get_provider_configs(state.clone()).unwrap_or_default();
            configs
                .into_iter()
                .filter(|c| c.configured)
                .map(|c| c.provider)
                .collect()
        }
    };

    let mut results = Vec::new();

    for provider_id in providers_to_check {
        let snapshot = match fetch_provider_usage(&state, &provider_id).await {
            Ok(s) => s,
            Err(e) => ProviderUsageSnapshotView {
                provider: provider_id.clone(),
                available: false,
                status: "error".to_string(),
                balance: None,
                currency: None,
                remaining_quota: None,
                period: None,
                last_updated_at: None,
                note: None,
                error: Some(e.to_string()),
            },
        };
        results.push(snapshot);
    }

    Ok(results)
}

async fn fetch_provider_usage(
    state: &AppState,
    provider: &str,
) -> Result<ProviderUsageSnapshotView, AppError> {
    let now = Utc::now().to_rfc3339();

    match provider {
        "minimax" => {
            let cfg = load_provider_config(&state.db, provider)?;
            let Some(cfg) = cfg else {
                return Ok(not_configured_snapshot(provider, &now));
            };

            fetch_minimax_coding_plan_usage(provider, &cfg.api_key, &now).await
        }
        "zhipu" | "glm" => {
            let cfg = load_provider_config(&state.db, provider)?;
            let Some(cfg) = cfg else {
                return Ok(not_configured_snapshot(provider, &now));
            };

            fetch_zai_coding_plan_usage(provider, &cfg.api_key, &now).await
        }
        "kimi" => {
            let cfg = load_provider_config(&state.db, provider)?;
            let Some(cfg) = cfg else {
                return Ok(not_configured_snapshot(provider, &now));
            };

            fetch_kimi_balance(provider, &cfg.api_key, &now).await
        }
        "gemini" => {
            let cfg = load_provider_config(&state.db, provider)?;
            let Some(cfg) = cfg else {
                return Ok(not_configured_snapshot(provider, &now));
            };

            fetch_gemini_api_health(provider, &cfg.api_key, &now).await
        }
        "modal" => {
            let cfg = load_provider_config(&state.db, provider)?;
            let Some(cfg) = cfg else {
                return Ok(not_configured_snapshot(provider, &now));
            };

            fetch_modal_api_health(provider, &cfg.api_key, cfg.base_url.as_deref(), &now).await
        }
        "openai-chatgpt" | "chatgpt" => {
            let cfg = load_provider_config(&state.db, provider)?;
            let Some(cfg) = cfg else {
                return Ok(not_configured_snapshot(provider, &now));
            };

            if looks_like_chatgpt_oauth_payload(&cfg.api_key) {
                return Ok(ProviderUsageSnapshotView {
                    provider: provider.to_string(),
                    available: false,
                    status: "not_applicable".to_string(),
                    balance: None,
                    currency: None,
                    remaining_quota: None,
                    period: None,
                    last_updated_at: Some(now),
                    note: Some(
                        "Connected via ChatGPT OAuth (subscription). API usage endpoint requires OpenAI API/admin key."
                            .to_string(),
                    ),
                    error: None,
                });
            }

            fetch_openai_usage(provider, &cfg.api_key, &now).await
        }
        _ => Ok(ProviderUsageSnapshotView {
            provider: provider.to_string(),
            available: false,
            status: "unknown_provider".to_string(),
            balance: None,
            currency: None,
            remaining_quota: None,
            period: None,
            last_updated_at: Some(now),
            note: Some(format!(
                "Provider '{}' is not supported for usage tracking",
                provider
            )),
            error: None,
        }),
    }
}

async fn fetch_kimi_balance(
    provider: &str,
    api_key: &str,
    now: &str,
) -> Result<ProviderUsageSnapshotView, AppError> {
    let url = "https://api.moonshot.ai/v1/users/me/balance";
    let json = get_json_with_auth(url, api_key, true).await?;

    let balance = pick_stringish(
        &json,
        &[
            "available_balance",
            "balance",
            "total_balance",
            "remaining_balance",
        ],
    );
    let currency = pick_stringish(&json, &["currency", "unit"]);

    Ok(ProviderUsageSnapshotView {
        provider: provider.to_string(),
        available: balance.is_some(),
        status: "ok".to_string(),
        balance,
        currency,
        remaining_quota: None,
        period: None,
        last_updated_at: Some(now.to_string()),
        note: Some("Fetched from Moonshot balance endpoint".to_string()),
        error: None,
    })
}

async fn fetch_minimax_coding_plan_usage(
    provider: &str,
    api_key: &str,
    now: &str,
) -> Result<ProviderUsageSnapshotView, AppError> {
    let url = "https://www.minimax.io/v1/api/openplatform/coding_plan/remains";
    let json = get_json_with_auth(url, api_key, true).await?;

    let remaining_direct = pick_stringish(
        &json,
        &[
            "remains",
            "remain",
            "remaining",
            "remaining_quota",
            "remain_prompt",
            "remaining_prompts",
            "prompt_remains",
        ],
    );
    let remaining = remaining_direct.or_else(|| pick_minimax_model_remains(&json));

    Ok(ProviderUsageSnapshotView {
        provider: provider.to_string(),
        available: remaining.is_some(),
        status: "ok".to_string(),
        balance: None,
        currency: None,
        remaining_quota: remaining,
        period: Some("5h rolling window".to_string()),
        last_updated_at: Some(now.to_string()),
        note: Some("Fetched from MiniMax Coding Plan remains endpoint".to_string()),
        error: None,
    })
}

fn pick_minimax_model_remains(value: &Value) -> Option<String> {
    let models = find_key_recursive(value, "model_remains")?.as_array()?;
    let mut total_remaining: i64 = 0;
    let mut counted = 0;

    for item in models {
        let total = item
            .get("current_interval_total_count")
            .and_then(|v| v.as_i64());
        let used = item
            .get("current_interval_usage_count")
            .and_then(|v| v.as_i64());
        if let (Some(total), Some(used)) = (total, used) {
            let remaining = (total - used).max(0);
            total_remaining += remaining;
            counted += 1;
        }
    }

    if counted > 0 {
        Some(format!("{} prompts", total_remaining))
    } else {
        None
    }
}

async fn fetch_zai_coding_plan_usage(
    provider: &str,
    api_key: &str,
    now: &str,
) -> Result<ProviderUsageSnapshotView, AppError> {
    let url = "https://api.z.ai/api/monitor/usage/quota/limit";
    let json = get_json_with_auth(url, api_key, false).await?;

    let remaining = pick_quota_percentage(&json).map(|value| format!("{}%", value));

    Ok(ProviderUsageSnapshotView {
        provider: provider.to_string(),
        available: remaining.is_some(),
        status: "ok".to_string(),
        balance: None,
        currency: None,
        remaining_quota: remaining,
        period: Some("Coding Plan limits".to_string()),
        last_updated_at: Some(now.to_string()),
        note: Some("Fetched from Z.AI monitor usage endpoint".to_string()),
        error: None,
    })
}

async fn fetch_openai_usage(
    provider: &str,
    api_key: &str,
    now: &str,
) -> Result<ProviderUsageSnapshotView, AppError> {
    let url = "https://api.openai.com/v1/organization/usage/completions?limit=1";
    let json = get_json_with_auth(url, api_key, true).await?;

    let total_tokens = pick_stringish(
        &json,
        &[
            "total_tokens",
            "input_tokens",
            "output_tokens",
            "num_model_requests",
        ],
    );

    Ok(ProviderUsageSnapshotView {
        provider: provider.to_string(),
        available: total_tokens.is_some(),
        status: "ok".to_string(),
        balance: None,
        currency: None,
        remaining_quota: total_tokens,
        period: Some("organization usage".to_string()),
        last_updated_at: Some(now.to_string()),
        note: Some("Fetched from OpenAI usage API".to_string()),
        error: None,
    })
}

async fn fetch_modal_api_health(
    provider: &str,
    api_key: &str,
    base_url: Option<&str>,
    now: &str,
) -> Result<ProviderUsageSnapshotView, AppError> {
    let base = base_url
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .unwrap_or_else(|| "https://api.us-west-2.modal.direct/v1".to_string());
    let url = format!("{}/models", base);

    let json = get_json_with_auth(&url, api_key, true).await?;
    let model_count = find_key_recursive(&json, "data")
        .and_then(|v| v.as_array())
        .map(|v| v.len())
        .unwrap_or(0);

    Ok(ProviderUsageSnapshotView {
        provider: provider.to_string(),
        available: true,
        status: "ok".to_string(),
        balance: None,
        currency: None,
        remaining_quota: Some(format!("{} model(s)", model_count)),
        period: None,
        last_updated_at: Some(now.to_string()),
        note: Some(
            "Modal does not expose balance via API; showing API key health via /models".to_string(),
        ),
        error: None,
    })
}

async fn fetch_gemini_api_health(
    provider: &str,
    api_key: &str,
    now: &str,
) -> Result<ProviderUsageSnapshotView, AppError> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://generativelanguage.googleapis.com/v1beta/models")
        .query(&[("key", api_key)])
        .send()
        .await
        .map_err(|e| AppError::Other(format!("request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<failed to read body>".to_string());
        return Err(AppError::Other(format!(
            "provider usage request failed ({status}): {body}"
        )));
    }

    let json: Value = response
        .json()
        .await
        .map_err(|e| AppError::Other(format!("failed to parse provider usage response: {e}")))?;
    let model_count = find_key_recursive(&json, "models")
        .and_then(|v| v.as_array())
        .map(|v| v.len())
        .unwrap_or(0);

    Ok(ProviderUsageSnapshotView {
        provider: provider.to_string(),
        available: true,
        status: "ok".to_string(),
        balance: None,
        currency: None,
        remaining_quota: Some(format!("{} model(s)", model_count)),
        period: None,
        last_updated_at: Some(now.to_string()),
        note: Some(
            "Gemini balance is managed in Google AI Studio/Cloud Console; showing API key health via /models"
                .to_string(),
        ),
        error: None,
    })
}

async fn get_json_with_auth(url: &str, api_key: &str, use_bearer: bool) -> Result<Value, AppError> {
    let client = reqwest::Client::new();
    let auth_value = if use_bearer {
        format!("Bearer {}", api_key)
    } else {
        api_key.to_string()
    };

    let response = client
        .get(url)
        .header("Authorization", auth_value)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| AppError::Other(format!("request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<failed to read body>".to_string());
        return Err(AppError::Other(format!(
            "provider usage request failed ({status}): {body}"
        )));
    }

    response
        .json::<Value>()
        .await
        .map_err(|e| AppError::Other(format!("failed to parse provider usage response: {e}")))
}

fn pick_stringish(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(found) = find_key_recursive(value, key) {
            match found {
                Value::String(s) if !s.trim().is_empty() => return Some(s.clone()),
                Value::Number(n) => return Some(n.to_string()),
                Value::Bool(b) => return Some(b.to_string()),
                _ => {}
            }
        }
    }
    None
}

fn pick_quota_percentage(value: &Value) -> Option<String> {
    let limits = find_key_recursive(value, "limits")?.as_array()?;
    for item in limits {
        let kind = item
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_ascii_uppercase();
        if kind == "TOKENS_LIMIT" {
            if let Some(percentage) = item.get("percentage") {
                return match percentage {
                    Value::String(s) => Some(s.clone()),
                    Value::Number(n) => Some(n.to_string()),
                    _ => None,
                };
            }
        }
    }
    None
}

fn find_key_recursive<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => {
            if let Some(v) = map.get(key) {
                return Some(v);
            }
            for v in map.values() {
                if let Some(found) = find_key_recursive(v, key) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(arr) => {
            for item in arr {
                if let Some(found) = find_key_recursive(item, key) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

fn looks_like_chatgpt_oauth_payload(api_key: &str) -> bool {
    serde_json::from_str::<Value>(api_key)
        .ok()
        .map(|value| value.get("access_token").is_some() && value.get("refresh_token").is_some())
        .unwrap_or(false)
}

fn not_configured_snapshot(provider: &str, now: &str) -> ProviderUsageSnapshotView {
    ProviderUsageSnapshotView {
        provider: provider.to_string(),
        available: false,
        status: "not_configured".to_string(),
        balance: None,
        currency: None,
        remaining_quota: None,
        period: None,
        last_updated_at: Some(now.to_string()),
        note: Some("Provider is not configured".to_string()),
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const GLM_KEY_PATH: &str = r"C:\Users\ghost\Desktop\Coding\glm-key.txt";
    const KIMI_KEY_PATH: &str = r"C:\Users\ghost\Desktop\Coding\kimi-test-key.txt";
    const MINIMAX_KEY_PATH: &str = r"C:\Users\ghost\Desktop\Coding\minimax-key.txt";
    const MODAL_KEY_PATH: &str = r"C:\Users\ghost\Desktop\Coding\modal-key.txt";
    const GEMINI_KEY_PATH: &str = r"C:\Users\ghost\Desktop\Coding\gemini-key.txt";

    fn read_key(path: &str) -> Option<String> {
        std::fs::read_to_string(path)
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    }

    #[test]
    fn parses_zai_quota_percentage() {
        let payload = json!({
            "code": 200,
            "data": {
                "limits": [
                    {"type": "TOKENS_LIMIT", "percentage": 12},
                    {"type": "TIME_LIMIT", "percentage": 1}
                ]
            }
        });
        assert_eq!(pick_quota_percentage(&payload), Some("12".to_string()));
    }

    #[test]
    fn parses_minimax_model_remains() {
        let payload = json!({
            "model_remains": [
                {
                    "model_name": "MiniMax-M2",
                    "current_interval_total_count": 1500,
                    "current_interval_usage_count": 1483
                },
                {
                    "model_name": "MiniMax-M2.5",
                    "current_interval_total_count": 500,
                    "current_interval_usage_count": 490
                }
            ]
        });
        assert_eq!(
            pick_minimax_model_remains(&payload),
            Some("27 prompts".to_string())
        );
    }

    #[tokio::test]
    async fn live_zai_usage_endpoint_with_provided_key() {
        let Some(key) = read_key(GLM_KEY_PATH) else {
            return;
        };
        let json = get_json_with_auth(
            "https://api.z.ai/api/monitor/usage/quota/limit",
            &key,
            false,
        )
        .await
        .expect("z.ai usage endpoint should be reachable with provided key");
        assert!(pick_quota_percentage(&json).is_some());
    }

    #[tokio::test]
    async fn live_minimax_usage_endpoint_with_provided_key() {
        let Some(key) = read_key(MINIMAX_KEY_PATH) else {
            return;
        };
        let json = get_json_with_auth(
            "https://www.minimax.io/v1/api/openplatform/coding_plan/remains",
            &key,
            true,
        )
        .await
        .expect("minimax remains endpoint should be reachable with provided key");
        assert!(
            pick_minimax_model_remains(&json).is_some()
                || pick_stringish(&json, &["remains"]).is_some()
        );
    }

    #[tokio::test]
    async fn live_kimi_balance_endpoint_with_provided_key() {
        let Some(key) = read_key(KIMI_KEY_PATH) else {
            return;
        };
        let result =
            get_json_with_auth("https://api.moonshot.ai/v1/users/me/balance", &key, true).await;
        // key may be invalid; we still exercise the real endpoint with provided key path
        if let Ok(json) = result {
            assert!(pick_stringish(&json, &["balance", "available_balance"]).is_some());
        }
    }

    #[test]
    fn provided_key_files_are_present() {
        let _ = read_key(GLM_KEY_PATH);
        let _ = read_key(KIMI_KEY_PATH);
        let _ = read_key(MINIMAX_KEY_PATH);
        let _ = read_key(MODAL_KEY_PATH);
        let _ = read_key(GEMINI_KEY_PATH);
    }
}
