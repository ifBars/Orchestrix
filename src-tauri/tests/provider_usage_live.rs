use reqwest::StatusCode;

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

#[tokio::test]
async fn zai_usage_endpoint_accepts_key_from_file() {
    let Some(key) = read_key(GLM_KEY_PATH) else {
        return;
    };

    let client = reqwest::Client::new();
    let response = client
        .get("https://api.z.ai/api/monitor/usage/quota/limit")
        .header("Authorization", key)
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("z.ai request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn minimax_remains_endpoint_accepts_key_from_file() {
    let Some(key) = read_key(MINIMAX_KEY_PATH) else {
        return;
    };

    let client = reqwest::Client::new();
    let response = client
        .get("https://www.minimax.io/v1/api/openplatform/coding_plan/remains")
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("minimax request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn kimi_balance_endpoint_is_exercised_with_key_from_file() {
    let Some(key) = read_key(KIMI_KEY_PATH) else {
        return;
    };

    let client = reqwest::Client::new();
    let response = client
        .get("https://api.moonshot.ai/v1/users/me/balance")
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("kimi request should succeed");

    assert!(
        response.status() == StatusCode::OK || response.status() == StatusCode::UNAUTHORIZED,
        "expected 200 or 401 from kimi balance endpoint"
    );
}

#[tokio::test]
async fn modal_models_endpoint_is_exercised_with_key_from_file() {
    let Some(key) = read_key(MODAL_KEY_PATH) else {
        return;
    };

    let client = reqwest::Client::new();
    let response = client
        .get("https://api.us-west-2.modal.direct/v1/models")
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("modal request should succeed");

    assert!(
        response.status() == StatusCode::OK
            || response.status() == StatusCode::UNAUTHORIZED
            || response.status() == StatusCode::FORBIDDEN
            || response.status() == StatusCode::NOT_FOUND,
        "unexpected modal status {}",
        response.status()
    );
}

#[tokio::test]
async fn gemini_models_endpoint_is_exercised_with_key_from_file() {
    let Some(key) = read_key(GEMINI_KEY_PATH) else {
        return;
    };

    let client = reqwest::Client::new();
    let response = client
        .get("https://generativelanguage.googleapis.com/v1beta/models")
        .query(&[("key", key)])
        .send()
        .await
        .expect("gemini request should succeed");

    assert!(
        response.status() == StatusCode::OK || response.status() == StatusCode::UNAUTHORIZED,
        "expected 200 or 401 from gemini models endpoint"
    );
}
