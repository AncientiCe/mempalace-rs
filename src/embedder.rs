//! Text embedding using fastembed (ONNX Runtime, all-MiniLM-L6-v2, 384-dim).
//!
//! Uses OnceLock for lazy initialization — commands that don't need embeddings
//! (e.g. `mempalace status`) start instantly without loading the model.
//! The model is downloaded on first use and cached in ~/.cache/huggingface.

use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use once_cell::sync::OnceCell;

/// Embedding dimension for all-MiniLM-L6-v2.
pub const EMBEDDING_DIM: usize = 384;

static EMBEDDER: OnceCell<TextEmbedding> = OnceCell::new();

fn get_embedder() -> Result<&'static TextEmbedding> {
    EMBEDDER.get_or_try_init(|| {
        TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2).with_show_download_progress(true),
        )
        .context("initializing fastembed (all-MiniLM-L6-v2)")
    })
}

/// Embed a single piece of text, returning a 384-dimensional vector.
pub fn embed_one(text: &str) -> Result<Vec<f32>> {
    let embedder = get_embedder()?;
    let mut results = embedder
        .embed(vec![text], None)
        .context("embedding text")?;
    results
        .pop()
        .ok_or_else(|| anyhow::anyhow!("embedding returned empty result"))
}

/// Embed multiple texts in one batch. Returns a Vec<Vec<f32>>.
pub fn embed_batch(texts: &[&str]) -> Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(vec![]);
    }
    let embedder = get_embedder()?;
    embedder
        .embed(texts.to_vec(), None)
        .context("batch embedding")
}

/// Serialize a f32 vector to little-endian bytes for SQLite BLOB storage.
pub fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Deserialize a BLOB back to a f32 vector.
pub fn blob_to_vec(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

/// Cosine similarity between two equal-length f32 vectors.
/// Returns a value in [-1.0, 1.0]; higher is more similar.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "vectors must have equal length");
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
