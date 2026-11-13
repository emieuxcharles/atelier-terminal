use std::{path::PathBuf, sync::mpsc, thread};

pub fn expand_path(input: &str) -> PathBuf {
    if input == "~" || input.starts_with("~/") {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
            .join(input.trim_start_matches('~').trim_start_matches('/'))
    } else {
        PathBuf::from(input)
    }
}

fn directories(input: &str) -> Result<Vec<String>, String> {
    let expanded = expand_path(input);
    let (parent, prefix, display_parent) = if input.ends_with('/') || input == "~" {
        (expanded, String::new(), format!("{}/", input.trim_end_matches('/')))
    } else {
        let prefix = expanded.file_name().unwrap_or_default().to_string_lossy().to_string();
        let parent = expanded.parent().unwrap_or(std::path::Path::new("/")).to_path_buf();
        let display_parent = input.rfind('/').map(|i| input[..=i].to_string()).unwrap_or_default();
        (parent, prefix, display_parent)
    };
    if !parent.is_absolute() { return Ok(vec![]); }
    let entries = std::fs::read_dir(parent).map_err(|_| "Impossible de lire ce dossier.".to_string())?;
    let mut matches = Vec::new();
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else { continue; };
        if name.starts_with('.') && !prefix.starts_with('.') { continue; }
        if name.to_lowercase().starts_with(&prefix.to_lowercase()) && entry.path().is_dir() {
            matches.push(format!("{display_parent}{name}/"));
        }
    }
    matches.sort_by_key(|path| path.to_lowercase());
    matches.truncate(100);
    Ok(matches)
}

pub struct Completion {
    request: mpsc::Sender<String>,
    response: mpsc::Receiver<(String, Result<Vec<String>, String>)>,
    input: Option<String>,
    pub suggestions: Vec<String>,
    pub selected: usize,
    pub error: Option<String>,
    pub loading: bool,
}

impl Completion {
    pub fn new(ctx: eframe::egui::Context) -> Self {
        let (request, requests) = mpsc::channel::<String>();
        let (results, response) = mpsc::channel();
        thread::spawn(move || {
            while let Ok(mut input) = requests.recv() {
                for newer in requests.try_iter() { input = newer; }
                let result = directories(&input);
                if results.send((input, result)).is_err() { break; }
                ctx.request_repaint();
            }
        });
        Self { request, response, input: None, suggestions: vec![], selected: 0, error: None, loading: false }
    }

    pub fn update(&mut self, input: &str) {
        if self.input.as_deref() != Some(input) {
            self.input = Some(input.to_string());
            self.suggestions.clear();
            self.error = None;
            self.selected = 0;
            self.loading = self.request.send(input.to_string()).is_ok();
        }
        for (query, result) in self.response.try_iter() {
            if query != input { continue; }
            self.loading = false;
            match result {
                Ok(paths) => self.suggestions = paths,
                Err(error) => self.error = Some(error),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completes_only_matching_directories_and_handles_spaces() {
        let root = std::env::temp_dir().join(format!("atelier-completion-{}", std::process::id()));
        std::fs::create_dir_all(root.join("Projet Alpha")).unwrap();
        std::fs::create_dir_all(root.join("Projet Beta")).unwrap();
        std::fs::create_dir_all(root.join(".prive")).unwrap();
        std::fs::write(root.join("Projet fichier"), "test").unwrap();
        let prefix = format!("{}/pro", root.display());
        assert_eq!(directories(&prefix).unwrap(), vec![
            format!("{}/Projet Alpha/", root.display()),
            format!("{}/Projet Beta/", root.display()),
        ]);
        assert_eq!(directories(&format!("{}/", root.display())).unwrap().len(), 2);
        assert_eq!(directories(&format!("{}/.p", root.display())).unwrap().len(), 1);
        assert!(directories(&format!("{}/absent/", root.display())).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
