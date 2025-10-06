use thiserror::Error;

#[derive(Error, Debug)]
pub enum VibeError {
    #[error("provider error: {0}")] Provider(String),
    #[error("schema error: {0}")] Schema(String),
    #[error("safety violation: {0}")] Safety(String),
    #[error("apply failed: {0}")] Apply(String),
}
