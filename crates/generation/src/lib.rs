mod clusterer;
mod llm;
mod orchestrator;

pub use orchestrator::{check_model_available, generate_drafts, OllamaConfig};

#[derive(thiserror::Error, Debug)]
pub enum GenerationError {
    #[error("LLM error: {0}")]
    Llm(String),
}
