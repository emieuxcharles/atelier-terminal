use serde::{Deserialize, Serialize};
use std::{io, path::{Path, PathBuf}};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme { #[default] System, Light, Dark }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedProject {
    pub directory: PathBuf,
    pub panes: usize,
    pub focused: usize,
    #[serde(default)]
    pub pane_directories: Vec<PathBuf>,
}

impl SavedProject {
    pub fn pane_directory(&self, index: usize) -> PathBuf {
        self.pane_directories.get(index).cloned().unwrap_or_else(|| self.directory.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub version: u32,
    pub compose_files: std::collections::BTreeMap<PathBuf,Vec<PathBuf>>,
    pub directory_history: Vec<crate::directory_history::Visit>,
    pub projects: Vec<SavedProject>,
    pub closed_projects: Vec<SavedProject>,
    pub selected: usize,
    pub font_size: f32,
    pub sidebar_visible: bool,
    pub theme: Theme,
}
impl Default for State {
    fn default() -> Self {
        Self { version: 1, compose_files: Default::default(), directory_history: vec![], projects: vec![], closed_projects: vec![], selected: 0, font_size: 15.0, sidebar_visible: true, theme: Theme::System }
    }
}

/// Oldest first; metadata only, never PTYs or terminal buffers.
pub fn remember_closed(history: &mut Vec<SavedProject>, project: SavedProject) {
    history.push(project);
    if history.len() > 10 { history.drain(..history.len() - 10); }
}

pub fn path() -> PathBuf {
    // Isolated state for manual verification, without touching personal projects.
    if let Some(path) = std::env::var_os("ATELIER_STATE_FILE") { return path.into(); }
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join("Library/Application Support/Atelier/state.json")
}

pub fn load(path: &Path) -> io::Result<Option<State>> {
    let data = match std::fs::read(path) {
        Ok(data) => data,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let mut state: State = serde_json::from_slice(&data)?;
    if state.version != 1 { return Err(io::Error::other("Version de sauvegarde non prise en charge")); }
    state.font_size = if state.font_size.is_finite() { state.font_size.clamp(11.0, 26.0) } else { 15.0 };
    state.selected = state.selected.min(state.projects.len().saturating_sub(1));
    if state.closed_projects.len() > 10 {
        state.closed_projects.drain(..state.closed_projects.len() - 10);
    }
    for project in state.projects.iter_mut().chain(state.closed_projects.iter_mut()) {
        project.panes = project.panes.max(1);
        project.focused = project.focused.min(project.panes - 1);
    }
    Ok(Some(state))
}

pub fn save(path: &Path, state: &State) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| io::Error::other("Chemin de sauvegarde invalide"))?;
    std::fs::create_dir_all(parent)?;
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    let result = (|| {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&temp)?;
        file.write_all(&serde_json::to_vec_pretty(state)?)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)
    })();
    if result.is_err() { let _ = std::fs::remove_file(temp); }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restores_each_pane_directory_and_supports_old_projects() {
        let old: SavedProject = serde_json::from_str(r#"{"directory":"/tmp/project","panes":2,"focused":1}"#).unwrap();
        assert_eq!(old.pane_directory(0), PathBuf::from("/tmp/project"));
        assert_eq!(old.pane_directory(1), PathBuf::from("/tmp/project"));
        let current = SavedProject { pane_directories: vec![PathBuf::from("/tmp/backend"), PathBuf::from("/tmp/frontend")], ..old };
        let encoded = serde_json::to_vec(&current).unwrap();
        let loaded: SavedProject = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(loaded.pane_directory(0), PathBuf::from("/tmp/backend"));
        assert_eq!(loaded.pane_directory(1), PathBuf::from("/tmp/frontend"));
    }
    #[test]
    fn history_keeps_latest_ten_in_reopen_order_and_survives_restart() {
        let root = std::env::temp_dir().join(format!("atelier-history-test-{}", std::process::id()));
        let path = root.join("state.json");
        let mut state = State::default();
        for index in 0..13 {
            remember_closed(&mut state.closed_projects, SavedProject {
                directory: PathBuf::from(format!("/tmp/project-{index}")), panes: 3, focused: 2, pane_directories: vec![],
            });
        }
        assert_eq!(state.closed_projects.len(), 10);
        save(&path, &state).unwrap();
        let mut restored = load(&path).unwrap().unwrap();
        for index in (3..13).rev() {
            let project = restored.closed_projects.pop().unwrap();
            assert_eq!(project.directory, PathBuf::from(format!("/tmp/project-{index}")));
            assert_eq!((project.panes, project.focused), (3, 2));
        }
        assert!(restored.closed_projects.is_empty());
        let legacy: State = serde_json::from_str(r#"{"version":1,"projects":[]}"#).unwrap();
        assert!(legacy.closed_projects.is_empty());
        assert!(legacy.directory_history.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn roundtrip_settings_projects_and_empty_workspace() {
        let root = std::env::temp_dir().join(format!("atelier-state-test-{}", std::process::id()));
        let path = root.join("state.json");
        assert!(load(&path).unwrap().is_none());
        let state = State {
            projects: vec![SavedProject { directory: PathBuf::from("/tmp/projet avec espaces"), panes: 5, focused: 4, pane_directories: vec![PathBuf::from("/tmp/backend"), PathBuf::from("/tmp/frontend")] }],
            directory_history: vec![crate::directory_history::Visit { directory: "/tmp/projet avec espaces".into(), visits: 7, last_used: 12345 }],
            font_size: 19.0, theme: Theme::Light, ..State::default()
        };
        save(&path, &state).unwrap();
        assert_eq!(load(&path).unwrap(), Some(state));
        save(&path, &State::default()).unwrap();
        assert!(load(&path).unwrap().unwrap().projects.is_empty());
        std::fs::write(&path, "invalid json").unwrap();
        assert!(load(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "invalid json");
        std::fs::remove_dir_all(root).unwrap();
    }
}
