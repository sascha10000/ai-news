mod clusterer;
mod llm;
mod orchestrator;

pub use orchestrator::{
    check_model_available, generate_drafts, generate_drafts_for_list, OllamaConfig,
};

#[derive(thiserror::Error, Debug)]
pub enum GenerationError {
    #[error("LLM error: {0}")]
    Llm(String),
}
