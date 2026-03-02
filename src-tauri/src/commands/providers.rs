use chrono::Utc;
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
            q.split('&')
                .find_map(|pair| {
                    let (k, v) = pair.split_once('=')?;
                    if k == "code" {
                        Some(urlencoding::decode(v).map(|s| s.into_owned()).unwrap_or_else(|_| v.to_owned()))
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
pub fn get_chatgpt_auth_status(state: tauri::State<'_, AppState>) -> Result<ChatGPTAuthStatus, AppError> {
    let setting = queries::get_setting(
        &state.db, "chatgpt_oauth_auth")
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
