use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::embeddings::error::EmbeddingError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingProviderKind {
    Remote,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EmbeddingTaskType {
    RetrievalQuery,
    RetrievalDocument,
    SemanticSimilarity,
    Classification,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmbedOptions {
    #[serde(default)]
    pub task: Option<EmbeddingTaskType>,
}

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    fn id(&self) -> &str;
    fn kind(&self) -> EmbeddingProviderKind;
    async fn dims(&self) -> Result<Option<usize>, EmbeddingError>;
    async fn embed(
        &self,
        texts: &[String],
        opts: Option<EmbedOptions>,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError>;
}

pub fn finalize_embeddings(
    mut vectors: Vec<Vec<f32>>,
    normalize_l2: bool,
) -> Result<Vec<Vec<f32>>, EmbeddingError> {
    if vectors.is_empty() {
        return Ok(vectors);
    }

    let dims = vectors[0].len();
    if dims == 0 {
        return Err(EmbeddingError::InvalidResponse(
            "provider returned an empty embedding vector".to_string(),
        ));
    }

    for (idx, vector) in vectors.iter().enumerate() {
        if vector.len() != dims {
            return Err(EmbeddingError::InvalidResponse(format!(
                "inconsistent embedding dimensions at index {idx}: expected {dims}, got {}",
                vector.len()
            )));
        }

        if vector.iter().any(|value| !value.is_finite()) {
            return Err(EmbeddingError::InvalidResponse(format!(
                "embedding vector at index {idx} contains non-finite values"
            )));
        }
    }

    if normalize_l2 {
        l2_normalize_in_place(&mut vectors);
    }

    Ok(vectors)
}

pub fn l2_normalize_in_place(vectors: &mut [Vec<f32>]) {
    for vector in vectors {
        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        if norm <= f32::EPSILON {
            continue;
        }
        for value in vector {
            *value /= norm;
        }
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (lhs, rhs) in a.iter().zip(b.iter()) {
        dot += lhs * rhs;
        norm_a += lhs * lhs;
        norm_b += rhs * rhs;
    }

    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom <= f32::EPSILON {
        return 0.0;
    }
    dot / denom
}
