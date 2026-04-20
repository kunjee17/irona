use crate::errors::IronaError;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct DeleteResult {
    pub index: usize,
    pub elapsed: Duration,
    pub outcome: Result<(), IronaError>,
}

pub async fn delete_all(paths: Vec<(usize, PathBuf)>) -> Vec<DeleteResult> {
    let handles: Vec<_> = paths
        .into_iter()
        .map(|(index, path)| {
            let handle = tokio::spawn(async move {
                let start = Instant::now();
                let outcome = tokio::fs::remove_dir_all(&path)
                    .await
                    .map_err(IronaError::from);
                DeleteResult {
                    index,
                    elapsed: start.elapsed(),
                    outcome,
                }
            });
            (handle, index)
        })
        .collect();

    let mut results = Vec::new();
    for (handle, index) in handles {
        match handle.await {
            Ok(r) => results.push(r),
            Err(_) => results.push(DeleteResult {
                index,
                elapsed: Duration::ZERO,
                outcome: Err(IronaError::ScanError("task panicked".to_string())),
            }),
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn deletes_existing_directory() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("artifacts");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("file.txt"), "data").unwrap();

        let results = delete_all(vec![(0, dir.clone())]).await;
        assert_eq!(results.len(), 1);
        assert!(results[0].outcome.is_ok());
        assert!(!dir.exists());
        assert!(results[0].elapsed.as_nanos() > 0);
    }

    #[tokio::test]
    async fn reports_error_for_missing_directory() {
        let results = delete_all(vec![(0, PathBuf::from("/nonexistent/xyz/abc"))]).await;
        assert_eq!(results.len(), 1);
        assert!(results[0].outcome.is_err());
    }

    #[tokio::test]
    async fn deletes_multiple_concurrently() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();

        let results = delete_all(vec![(0, a.clone()), (1, b.clone())]).await;
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.outcome.is_ok()));
        assert!(!a.exists());
        assert!(!b.exists());
    }
}
