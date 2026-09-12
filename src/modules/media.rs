pub mod ai;
pub mod config;
pub mod heuristic;
pub mod organizer;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaType {
    Movie,
    Show,
}

impl std::fmt::Display for MediaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Movie => write!(f, "Movie"),
            Self::Show => write!(f, "TV Show"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClassificationEngine {
    Ai,
    Heuristic,
}

impl std::fmt::Display for ClassificationEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ai => write!(f, "🤖 Gemini AI"),
            Self::Heuristic => write!(f, "⚡ Heuristic Fallback"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    pub media_type: MediaType,
    pub title: String,
    pub year: Option<u32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub resolution: Option<String>,
    pub clean_name: String,
    pub engine: ClassificationEngine,
}

#[derive(Debug, Clone)]
pub struct OrganizeResult {
    pub source_path: PathBuf,
    pub dest_path: PathBuf,
    pub media_info: MediaInfo,
}
