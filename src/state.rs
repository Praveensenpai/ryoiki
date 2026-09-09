use anyhow::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

/// Persists provisioning run progress so interrupted runs can be resumed.
#[derive(Serialize, Deserialize, Default)]
pub struct RunState {
    pub completed: Vec<String>,
    pub pending: Vec<String>,
}

fn state_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    Path::new(&home).join(".local/state/ryoiki/resume.json")
}

impl RunState {
    /// Loads a previously saved state from disk, if one exists.
    pub fn load() -> Option<Self> {
        let path = state_path();
        let raw = fs::read_to_string(&path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Persists the current state to disk.
    pub fn save(&self) -> Result<()> {
        let path = state_path();
        let json = serde_json::to_string_pretty(self).context("Failed to serialize run state")?;
        fs::write(&path, json).context("Failed to write resume.json")?;
        Ok(())
    }

    /// Removes the state file after a clean finish.
    pub fn clear() {
        let _ = fs::remove_file(state_path());
    }

    /// Records a module as successfully completed and persists state.
    pub fn mark_done(&mut self, module_id: &str) -> Result<()> {
        self.completed.push(module_id.to_string());
        self.pending.retain(|m| m != module_id);
        self.save()
    }
}

/// Checks for a stale resume file and prompts the user to resume or restart.
/// Returns the filtered module list and the active `RunState`.
pub fn resolve_resume(
    module_ids: Vec<String>,
    non_interactive: bool,
) -> Result<(Vec<String>, RunState)> {
    let Some(saved) = RunState::load() else {
        let state = RunState {
            completed: vec![],
            pending: module_ids.clone(),
        };
        state.save()?;
        return Ok((module_ids, state));
    };

    if saved.completed.is_empty() {
        return Ok((module_ids, saved));
    }

    let resume = if non_interactive {
        true
    } else {
        prompt_resume(&saved.completed)?
    };

    if resume {
        let remaining: Vec<String> = module_ids
            .into_iter()
            .filter(|id| !saved.completed.contains(id))
            .collect();
        Ok((remaining, saved))
    } else {
        RunState::clear();
        let fresh = RunState {
            completed: vec![],
            pending: module_ids.clone(),
        };
        fresh.save()?;
        Ok((module_ids, fresh))
    }
}

fn prompt_resume(completed: &[String]) -> Result<bool> {
    println!(
        "\n  {} Interrupted run detected ({} module(s) already done).",
        "⚡".yellow(),
        completed.len()
    );
    print!("  Resume from where it left off? [Y/n]: ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    let trimmed = answer.trim().to_lowercase();
    Ok(trimmed.is_empty() || trimmed == "y" || trimmed == "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_state_serialization() -> Result<()> {
        let state = RunState {
            completed: vec!["docker".to_string()],
            pending: vec!["caddy".to_string()],
        };
        let json = serde_json::to_string(&state)?;
        let loaded: RunState = serde_json::from_str(&json)?;
        assert_eq!(loaded.completed, vec!["docker"]);
        assert_eq!(loaded.pending, vec!["caddy"]);
        Ok(())
    }

    #[test]
    fn test_mark_done_updates_in_memory() {
        let mut state = RunState {
            completed: vec![],
            pending: vec!["docker".to_string(), "caddy".to_string()],
        };
        state.completed.push("docker".to_string());
        state.pending.retain(|m| m != "docker");
        assert_eq!(state.completed, vec!["docker"]);
        assert_eq!(state.pending, vec!["caddy"]);
    }
}
