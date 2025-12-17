//! Corpus loading utilities for spike tests
//!
//! Loads markdown files from test-corpora submodule and converts
//! them to Plexus ContentItem structures for analysis.

use plexus::{ContentItem, ContentType};
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

/// A loaded test corpus with metadata
#[derive(Debug)]
pub struct TestCorpus {
    /// Name of the corpus (e.g., "pkm-webdev")
    pub name: String,
    /// Root path of the corpus
    pub root_path: PathBuf,
    /// All content items loaded from the corpus
    pub items: Vec<ContentItem>,
    /// Number of files loaded
    pub file_count: usize,
}

impl TestCorpus {
    /// Load a corpus from the test-corpora submodule
    ///
    /// # Arguments
    /// * `corpus_name` - Name of the corpus directory (e.g., "pkm-webdev")
    ///
    /// # Returns
    /// A TestCorpus containing all markdown files from the corpus
    pub fn load(corpus_name: &str) -> Result<Self, CorpusError> {
        let root = corpus_root().join(corpus_name);
        if !root.exists() {
            return Err(CorpusError::NotFound(corpus_name.to_string()));
        }

        let items = load_markdown_files(&root)?;
        let file_count = items.len();

        Ok(Self {
            name: corpus_name.to_string(),
            root_path: root,
            items,
            file_count,
        })
    }

    /// Get items in a specific directory (relative to corpus root)
    pub fn items_in_directory(&self, dir: &str) -> Vec<&ContentItem> {
        self.items
            .iter()
            .filter(|item| {
                item.path
                    .as_ref()
                    .map(|p| p.starts_with(dir))
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Get all directory names in the corpus
    pub fn directories(&self) -> Vec<String> {
        let mut dirs: std::collections::HashSet<String> = std::collections::HashSet::new();

        for item in &self.items {
            if let Some(path) = &item.path {
                if let Some(parent) = path.parent() {
                    let parent_str = parent.to_string_lossy().to_string();
                    if !parent_str.is_empty() {
                        dirs.insert(parent_str);
                    }
                }
            }
        }

        dirs.into_iter().collect()
    }

    /// Get a content item by its relative path
    pub fn get_by_path(&self, relative_path: &str) -> Option<&ContentItem> {
        self.items.iter().find(|item| {
            item.path
                .as_ref()
                .map(|p| p.to_string_lossy() == relative_path)
                .unwrap_or(false)
        })
    }

    /// Get all README files in the corpus
    pub fn readme_files(&self) -> Vec<&ContentItem> {
        self.items
            .iter()
            .filter(|item| {
                item.path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_lowercase().contains("readme"))
                    .unwrap_or(false)
            })
            .collect()
    }
}

/// Get the test-corpora root directory
pub fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-corpora")
}

/// Load all text files (markdown and plain text) recursively from a directory
fn load_markdown_files(root: &Path) -> Result<Vec<ContentItem>, CorpusError> {
    let mut items = Vec::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "md" || ext == "txt")
                .unwrap_or(false)
        })
    {
        let path = entry.path();
        let content = std::fs::read_to_string(path)
            .map_err(|e| CorpusError::ReadError(path.to_path_buf(), e))?;

        // Use relative path for the ContentItem
        let relative_path = path.strip_prefix(root).unwrap_or(path).to_path_buf();

        items.push(ContentItem::from_file(
            relative_path,
            ContentType::Document,
            content,
        ));
    }

    Ok(items)
}

#[derive(Debug, Error)]
pub enum CorpusError {
    #[error("Corpus not found: {0}")]
    NotFound(String),
    #[error("Failed to read {0}: {1}")]
    ReadError(PathBuf, std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_corpus_root_exists() {
        let root = corpus_root();
        assert!(root.exists(), "test-corpora submodule not initialized");
    }

    #[test]
    fn test_load_pkm_webdev() {
        let corpus = TestCorpus::load("pkm-webdev");
        assert!(corpus.is_ok(), "Failed to load pkm-webdev corpus");

        let corpus = corpus.unwrap();
        assert!(corpus.file_count > 0, "pkm-webdev should have files");
    }
}
