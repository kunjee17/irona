use std::path::PathBuf;

#[derive(Debug)]
#[allow(dead_code)]
pub struct DeleteResult {
    pub path: PathBuf,
    pub success: bool,
    pub error: Option<String>,
}

pub async fn delete_all(paths: Vec<PathBuf>) -> Vec<DeleteResult> {
    let handles: Vec<_> = paths
        .into_iter()
        .map(|path| {
            tokio::spawn(async move {
                match tokio::fs::remove_dir_all(&path).await {
                    Ok(_) => DeleteResult {
                        path,
                        success: true,
                        error: None,
                    },
                    Err(e) => DeleteResult {
                        path,
                        success: false,
                        error: Some(e.to_string()),
                    },
                }
            })
        })
        .collect();

    let mut results = Vec::new();
    for handle in handles {
        if let Ok(result) = handle.await {
            results.push(result);
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

        let results = delete_all(vec![dir.clone()]).await;
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert!(!dir.exists());
    }

    #[tokio::test]
    async fn reports_error_for_missing_directory() {
        let results = delete_all(vec![PathBuf::from("/nonexistent/xyz/abc")]).await;
        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert!(results[0].error.is_some());
    }

    #[tokio::test]
    async fn deletes_multiple_concurrently() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();

        let results = delete_all(vec![a.clone(), b.clone()]).await;
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.success));
        assert!(!a.exists());
        assert!(!b.exists());
    }
}
