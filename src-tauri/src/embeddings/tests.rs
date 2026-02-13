use std::sync::{Arc, Mutex};

use httpmock::Method::POST;
use httpmock::MockServer;
use serde_json::json;

use crate::embeddings::config::{
    EmbeddingConfig, EmbeddingProviderId, GeminiEmbeddingConfig, OllamaEmbeddingConfig,
    TransformersJsEmbeddingConfig,
};
use crate::embeddings::providers::{
    GeminiEmbeddingProvider, OllamaEmbeddingProvider, RustHfEngine, RustLocalEmbeddingProvider,
    TransformersBridgeRequest, TransformersBridgeTransport, TransformersJsEmbeddingProvider,
};
use crate::embeddings::types::{EmbedOptions, EmbeddingProvider, EmbeddingTaskType};

#[tokio::test]
async fn gemini_provider_builds_expected_request_and_shape() {
    let server = MockServer::start();
    let path = "/v1beta/models/gemini-embedding-001:batchEmbedContents";
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path(path)
            .query_param("key", "test-key")
            .header("content-type", "application/json")
            .body_contains("\"taskType\":\"RETRIEVAL_QUERY\"")
            .body_contains("\"content\"")
            .body_contains("\"requests\"");
        then.status(200).json_body(json!({
            "embeddings": [
                { "values": [1.0, 0.0, 0.0] },
                { "values": [0.0, 1.0, 0.0] }
            ]
        }));
    });

    let provider = GeminiEmbeddingProvider::new(
        GeminiEmbeddingConfig {
            api_key: Some("test-key".to_string()),
            model: "gemini-embedding-001".to_string(),
            timeout_ms: 5_000,
            base_url: Some(format!("{}/v1beta", server.base_url())),
        },
        Some("test-key".to_string()),
        false,
    )
    .expect("gemini provider should initialize");

    let vectors = provider
        .embed(
            &["alpha".to_string(), "beta".to_string()],
            Some(EmbedOptions {
                task: Some(EmbeddingTaskType::RetrievalQuery),
            }),
        )
        .await
        .expect("gemini embed should succeed");

    mock.assert();
    assert_eq!(vectors.len(), 2);
    assert_eq!(vectors[0].len(), 3);
}

#[tokio::test]
async fn ollama_provider_uses_batch_endpoint_when_available() {
    let server = MockServer::start();
    let batch = server.mock(|when, then| {
        when.method(POST)
            .path("/api/embed")
            .body_contains("\"model\":\"nomic-embed-text\"")
            .body_contains("\"input\"");
        then.status(200).json_body(json!({
            "embeddings": [
                [0.1, 0.2, 0.3],
                [0.4, 0.5, 0.6]
            ]
        }));
    });

    let provider = OllamaEmbeddingProvider::new(
        OllamaEmbeddingConfig {
            base_url: server.base_url(),
            model: "nomic-embed-text".to_string(),
            timeout_ms: 5_000,
        },
        false,
    )
    .expect("ollama provider should initialize");

    let vectors = provider
        .embed(&["alpha".to_string(), "beta".to_string()], None)
        .await
        .expect("ollama embed should succeed");

    batch.assert();
    assert_eq!(vectors.len(), 2);
    assert_eq!(vectors[0].len(), 3);
}

#[tokio::test]
async fn ollama_provider_falls_back_to_legacy_endpoint() {
    let server = MockServer::start();
    let _batch = server.mock(|when, then| {
        when.method(POST).path("/api/embed");
        then.status(404).json_body(json!({ "error": "not found" }));
    });
    let legacy = server.mock(|when, then| {
        when.method(POST)
            .path("/api/embeddings")
            .body_contains("\"prompt\"");
        then.status(200)
            .json_body(json!({ "embedding": [0.7, 0.8, 0.9] }));
    });

    let provider = OllamaEmbeddingProvider::new(
        OllamaEmbeddingConfig {
            base_url: server.base_url(),
            model: "nomic-embed-text".to_string(),
            timeout_ms: 5_000,
        },
        false,
    )
    .expect("ollama provider should initialize");

    let vectors = provider
        .embed(&["alpha".to_string(), "beta".to_string()], None)
        .await
        .expect("ollama fallback should succeed");

    assert_eq!(vectors.len(), 2);
    assert_eq!(vectors[0].len(), 3);
    assert!(legacy.hits() >= 2);
}

#[tokio::test]
async fn transformers_provider_uses_transport_contract() {
    let transport = Arc::new(MockTransformersTransport::default());
    let provider = TransformersJsEmbeddingProvider::new_with_transport(
        TransformersJsEmbeddingConfig {
            model: "Xenova/all-MiniLM-L6-v2".to_string(),
            device: "cpu".to_string(),
            backend: None,
            cache_dir: None,
            timeout_ms: 10_000,
            bridge_command: "bun".to_string(),
            bridge_script: None,
        },
        false,
        transport.clone(),
    )
    .expect("transformers provider should initialize");

    let dims = provider.dims().await.expect("dims should succeed");
    assert_eq!(dims, Some(3));

    let vectors = provider
        .embed(&["alpha".to_string(), "beta".to_string()], None)
        .await
        .expect("embed should succeed");
    assert_eq!(vectors.len(), 2);
    assert_eq!(vectors[0], vec![1.0, 0.0, 0.0]);

    let calls = transport.calls.lock().expect("calls mutex").clone();
    assert!(calls.iter().any(|call| call.action == "dims"));
    assert!(calls.iter().any(|call| call.action == "embed"));
    assert!(calls
        .iter()
        .all(|call| call.request.model == "Xenova/all-MiniLM-L6-v2"));
}

#[tokio::test]
async fn rust_hf_provider_uses_engine_contract() {
    let engine = Arc::new(MockRustHfEngine::default());
    let provider = RustLocalEmbeddingProvider::new_with_engine(engine.clone(), true);

    let dims = provider.dims().await.expect("dims should succeed");
    assert_eq!(dims, Some(3));

    let vectors = provider
        .embed(&["alpha".to_string(), "beta".to_string()], None)
        .await
        .expect("embed should succeed");
    assert_eq!(vectors.len(), 2);
    assert_eq!(vectors[0].len(), 3);

    let calls = engine.calls.lock().expect("calls mutex").clone();
    assert!(calls.iter().any(|value| value == "dims"));
    assert!(calls.iter().any(|value| value == "embed:2"));
}

#[tokio::test]
async fn provider_agnostic_integration_suite_mock_default() {
    let gemini_server = MockServer::start();
    let _gemini = gemini_server.mock(|when, then| {
        when.method(POST)
            .path("/v1beta/models/gemini-embedding-001:batchEmbedContents")
            .query_param("key", "mock-gemini");
        then.status(200).json_body(json!({
            "embeddings": [
                { "values": [1.0, 0.0, 0.0] }
            ]
        }));
    });

    let ollama_server = MockServer::start();
    let _ollama = ollama_server.mock(|when, then| {
        when.method(POST).path("/api/embed");
        then.status(200).json_body(json!({
            "embeddings": [
                [0.1, 0.0, 0.0]
            ]
        }));
    });

    let gemini_provider = GeminiEmbeddingProvider::new(
        GeminiEmbeddingConfig {
            api_key: Some("mock-gemini".to_string()),
            model: "gemini-embedding-001".to_string(),
            timeout_ms: 5_000,
            base_url: Some(format!("{}/v1beta", gemini_server.base_url())),
        },
        Some("mock-gemini".to_string()),
        false,
    )
    .expect("gemini provider should initialize");

    let ollama_provider = OllamaEmbeddingProvider::new(
        OllamaEmbeddingConfig {
            base_url: ollama_server.base_url(),
            model: "nomic-embed-text".to_string(),
            timeout_ms: 5_000,
        },
        false,
    )
    .expect("ollama provider should initialize");

    let transformers_provider = TransformersJsEmbeddingProvider::new_with_transport(
        TransformersJsEmbeddingConfig {
            model: "Xenova/all-MiniLM-L6-v2".to_string(),
            device: "cpu".to_string(),
            backend: None,
            cache_dir: None,
            timeout_ms: 10_000,
            bridge_command: "bun".to_string(),
            bridge_script: None,
        },
        false,
        Arc::new(MockTransformersTransport::default()),
    )
    .expect("transformers provider should initialize");

    let rust_provider =
        RustLocalEmbeddingProvider::new_with_engine(Arc::new(MockRustHfEngine::default()), false);

    assert_provider_contract(&gemini_provider).await;
    assert_provider_contract(&ollama_provider).await;
    assert_provider_contract(&transformers_provider).await;
    assert_provider_contract(&rust_provider).await;
}

#[tokio::test]
async fn optional_real_ollama_provider_when_enabled() {
    if std::env::var("RUN_REAL_OLLAMA").ok().as_deref() != Some("1") {
        return;
    }

    let mut config = EmbeddingConfig::default();
    config.provider = EmbeddingProviderId::Ollama;
    config.ollama.timeout_ms = 30_000;
    config
        .validate_selected_provider()
        .expect("ollama config should validate");

    let provider = OllamaEmbeddingProvider::new(config.ollama.clone(), false)
        .expect("ollama provider should initialize");
    assert_provider_contract(&provider).await;
}

#[tokio::test]
async fn optional_real_gemini_provider_when_enabled() {
    if std::env::var("RUN_REAL_GEMINI").ok().as_deref() != Some("1") {
        return;
    }

    let api_key = std::env::var("GEMINI_API_KEY")
        .or_else(|_| std::env::var("GOOGLE_API_KEY"))
        .expect("RUN_REAL_GEMINI=1 requires GEMINI_API_KEY or GOOGLE_API_KEY");

    let provider = GeminiEmbeddingProvider::new(
        GeminiEmbeddingConfig {
            api_key: Some(api_key.clone()),
            model: "gemini-embedding-001".to_string(),
            timeout_ms: 30_000,
            base_url: None,
        },
        Some(api_key),
        false,
    )
    .expect("gemini provider should initialize");

    assert_provider_contract(&provider).await;
}

async fn assert_provider_contract(provider: &dyn EmbeddingProvider) {
    let empty = provider
        .embed(&Vec::new(), None)
        .await
        .expect("empty input should succeed");
    assert!(empty.is_empty());

    let inputs = vec!["alpha content for testing".to_string()];
    let vectors = provider
        .embed(
            &inputs,
            Some(EmbedOptions {
                task: Some(EmbeddingTaskType::RetrievalDocument),
            }),
        )
        .await
        .expect("embed should succeed");
    assert_eq!(vectors.len(), inputs.len());

    let dims = vectors[0].len();
    assert!(dims > 0);
    assert!(vectors.iter().all(|vector| vector.len() == dims));

    let provider_dims = provider.dims().await.expect("dims should succeed");
    if let Some(value) = provider_dims {
        assert_eq!(value, dims);
    }

    let vectors_repeat = provider
        .embed(
            &inputs,
            Some(EmbedOptions {
                task: Some(EmbeddingTaskType::RetrievalDocument),
            }),
        )
        .await
        .expect("repeated embed should succeed");
    assert_eq!(vectors_repeat.len(), vectors.len());
    assert_eq!(vectors_repeat[0].len(), vectors[0].len());

    let long_input = vec!["x".repeat(9_000)];
    let long_vectors = provider
        .embed(
            &long_input,
            Some(EmbedOptions {
                task: Some(EmbeddingTaskType::RetrievalDocument),
            }),
        )
        .await
        .expect("long input embed should succeed");
    assert_eq!(long_vectors.len(), 1);
    assert_eq!(long_vectors[0].len(), dims);
}

#[derive(Clone)]
struct RecordedTransformersCall {
    action: String,
    request: TransformersBridgeRequest,
}

#[derive(Default)]
struct MockTransformersTransport {
    calls: Mutex<Vec<RecordedTransformersCall>>,
}

#[async_trait::async_trait]
impl TransformersBridgeTransport for MockTransformersTransport {
    async fn dims(
        &self,
        request: &TransformersBridgeRequest,
    ) -> Result<Option<usize>, crate::embeddings::EmbeddingError> {
        self.calls
            .lock()
            .expect("calls mutex")
            .push(RecordedTransformersCall {
                action: "dims".to_string(),
                request: request.clone(),
            });
        Ok(Some(3))
    }

    async fn embed(
        &self,
        request: &TransformersBridgeRequest,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, crate::embeddings::EmbeddingError> {
        self.calls
            .lock()
            .expect("calls mutex")
            .push(RecordedTransformersCall {
                action: "embed".to_string(),
                request: request.clone(),
            });

        Ok(texts
            .iter()
            .enumerate()
            .map(|(index, _)| {
                if index % 2 == 0 {
                    vec![1.0, 0.0, 0.0]
                } else {
                    vec![0.0, 1.0, 0.0]
                }
            })
            .collect())
    }
}

#[derive(Default)]
struct MockRustHfEngine {
    calls: Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl RustHfEngine for MockRustHfEngine {
    async fn dims(&self) -> Result<Option<usize>, crate::embeddings::EmbeddingError> {
        self.calls
            .lock()
            .expect("calls mutex")
            .push("dims".to_string());
        Ok(Some(3))
    }

    async fn embed(
        &self,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, crate::embeddings::EmbeddingError> {
        self.calls
            .lock()
            .expect("calls mutex")
            .push(format!("embed:{}", texts.len()));

        Ok(texts
            .iter()
            .enumerate()
            .map(|(index, _)| {
                if index % 2 == 0 {
                    vec![0.0, 1.0, 0.0]
                } else {
                    vec![1.0, 0.0, 0.0]
                }
            })
            .collect())
    }
}
