//! Storage for optimization results.
//!
//! Persists optimization results to disk for later analysis.

use std::path::Path;
use std::fs;
use serde::{Deserialize, Serialize};
use crate::grid_search::OptimizationReport;
use crate::walk_forward::WalkForwardSummary;
use crate::OptimizationResult;

/// Stored optimization run metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationRun {
    /// Unique identifier for this run.
    pub id: String,
    /// Timestamp of the run.
    pub timestamp: String,
    /// Strategy name.
    pub strategy_name: String,
    /// Grid search report.
    pub grid_search: Option<OptimizationReport>,
    /// Walk-forward summary.
    pub walk_forward: Option<WalkForwardSummary>,
}

/// Storage for optimization results.
#[derive(Debug, Clone)]
pub struct OptimizationStorage {
    /// Base directory for storing results.
    base_dir: String,
}

impl OptimizationStorage {
    /// Create a new storage instance.
    pub fn new(base_dir: impl Into<String>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Save an optimization run.
    pub fn save(&self, run: &OptimizationRun) -> OptimizationResult<()> {
        let dir = Path::new(&self.base_dir);
        fs::create_dir_all(dir)?;

        let filename = format!("{}.json", run.id);
        let path = dir.join(&filename);
        let content = serde_json::to_string_pretty(run)?;

        fs::write(path, content)?;
        Ok(())
    }

    /// Load an optimization run by ID.
    pub fn load(&self, id: &str) -> OptimizationResult<OptimizationRun> {
        let path = Path::new(&self.base_dir).join(format!("{}.json", id));
        let content = fs::read_to_string(path)?;
        let run = serde_json::from_str(&content)?;
        Ok(run)
    }

    /// List all stored run IDs.
    pub fn list(&self) -> OptimizationResult<Vec<String>> {
        let dir = Path::new(&self.base_dir);
        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut ids = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Some(stem) = path.file_stem() {
                    ids.push(stem.to_string_lossy().to_string());
                }
            }
        }

        ids.sort();
        Ok(ids)
    }

    /// Delete a stored run by ID.
    pub fn delete(&self, id: &str) -> OptimizationResult<()> {
        let path = Path::new(&self.base_dir).join(format!("{}.json", id));
        fs::remove_file(path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_storage_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let storage = OptimizationStorage::new(temp_dir.path().to_string_lossy().to_string());

        let run = OptimizationRun {
            id: "test-run-1".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            strategy_name: "sma_crossover".to_string(),
            grid_search: None,
            walk_forward: None,
        };

        storage.save(&run).unwrap();
        let loaded = storage.load("test-run-1").unwrap();
        assert_eq!(loaded.id, "test-run-1");
        assert_eq!(loaded.strategy_name, "sma_crossover");
    }

    #[test]
    fn test_list_runs() {
        let temp_dir = TempDir::new().unwrap();
        let storage = OptimizationStorage::new(temp_dir.path().to_string_lossy().to_string());

        let run1 = OptimizationRun {
            id: "run-1".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            strategy_name: "test".to_string(),
            grid_search: None,
            walk_forward: None,
        };

        let run2 = OptimizationRun {
            id: "run-2".to_string(),
            timestamp: "2024-01-02T00:00:00Z".to_string(),
            strategy_name: "test".to_string(),
            grid_search: None,
            walk_forward: None,
        };

        storage.save(&run1).unwrap();
        storage.save(&run2).unwrap();

        let ids = storage.list().unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"run-1".to_string()));
        assert!(ids.contains(&"run-2".to_string()));
    }

    #[test]
    fn test_delete_run() {
        let temp_dir = TempDir::new().unwrap();
        let storage = OptimizationStorage::new(temp_dir.path().to_string_lossy().to_string());

        let run = OptimizationRun {
            id: "to-delete".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            strategy_name: "test".to_string(),
            grid_search: None,
            walk_forward: None,
        };

        storage.save(&run).unwrap();
        storage.delete("to-delete").unwrap();
        assert!(storage.load("to-delete").is_err());
    }
}
