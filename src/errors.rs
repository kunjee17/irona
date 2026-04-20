use std::sync::Arc;

#[derive(Debug, Clone, thiserror::Error)]
pub enum IronaError {
    #[error("delete failed: {0}")]
    DeleteFailed(Arc<std::io::Error>),

    #[error("scan error: {0}")]
    ScanError(String),
}

impl PartialEq for IronaError {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}

impl From<std::io::Error> for IronaError {
    fn from(e: std::io::Error) -> Self {
        Self::DeleteFailed(Arc::new(e))
    }
}
