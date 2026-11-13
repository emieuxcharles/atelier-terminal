use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::{SystemTime, UNIX_EPOCH}, os::unix::ffi::OsStringExt};

pub const MAX_ENTRIES: usize = 2000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Visit {
    pub directory: PathBuf,
    pub visits: u64,
    pub last_used: u64,
}

fn valid(path: &std::path::Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let bytes = path.as_os_str().as_bytes();
    path.is_absolute() && path.to_str().is_some() && bytes.len() <= 8192 && !bytes.contains(&0)
}

fn rank(entry: &Visit, now: u64) -> u64 {
    let age = now.saturating_sub(entry.last_used);
    let weight = if age < 3600 { 16 } else if age < 86400 { 8 } else if age < 604800 { 2 } else { 1 };
    entry.visits.saturating_mul(weight)
}

pub fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }

#[derive(Default)]
pub struct History { entries: Vec<Visit>, pub dirty: bool }

impl History {
    pub fn restore(entries: Vec<Visit>) -> Self {
        let mut merged = std::collections::BTreeMap::<PathBuf, Visit>::new();
        for mut entry in entries.into_iter().filter(|v| valid(&v.directory)) {
            entry.visits = entry.visits.clamp(1, 1_000_000);
            merged.entry(entry.directory.clone()).and_modify(|old| {
                old.visits = old.visits.saturating_add(entry.visits).min(1_000_000);
                old.last_used = old.last_used.max(entry.last_used);
            }).or_insert(entry);
        }
        let mut entries: Vec<_> = merged.into_values().collect();
        entries.sort_by_key(|entry| std::cmp::Reverse((rank(entry, now()), entry.last_used)));
        entries.truncate(MAX_ENTRIES);
        Self { entries, dirty: false }
    }
    pub fn entries(&self) -> &[Visit] { &self.entries }
    pub fn seed(&mut self, path: PathBuf, now: u64) {
        if !self.entries.iter().any(|e| e.directory == path) { self.visit(path, now); }
    }
    pub fn visit(&mut self, path: PathBuf, now: u64) {
        if !valid(&path) { return; }
        if let Some(entry) = self.entries.iter_mut().find(|e| e.directory == path) {
            entry.visits = entry.visits.saturating_add(1).min(1_000_000);
            entry.last_used = now;
        } else {
            if self.entries.len() == MAX_ENTRIES {
                let oldest = self.entries.iter().enumerate().min_by_key(|(_, e)| (rank(e, now), e.last_used)).unwrap().0;
                self.entries.swap_remove(oldest);
            }
            self.entries.push(Visit { directory: path, visits: 1, last_used: now });
        }
        self.dirty = true;
    }
    pub fn ranked(&self, now: u64) -> Vec<PathBuf> {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by(|a, b| rank(b, now).cmp(&rank(a, now)).then_with(|| b.last_used.cmp(&a.last_used)).then_with(|| a.directory.cmp(&b.directory)));
        entries.into_iter().map(|e| e.directory.clone()).collect()
    }
}

/// A local zsh hook emits physical paths as hex, preserving spaces and control bytes.
pub fn parse_cwd(title: &str, token: &str) -> Option<PathBuf> {
    let encoded = title.strip_prefix("ATELIER_CWD:")?.strip_prefix(token)?.strip_prefix(':')?;
    if encoded.len() > 16384 || encoded.len() % 2 != 0 { return None; }
    let mut bytes = Vec::with_capacity(encoded.len() / 2);
    for pair in encoded.as_bytes().chunks_exact(2) {
        let digit = |b: u8| (b as char).to_digit(16).map(|n| n as u8);
        bytes.push(digit(pair[0])? * 16 + digit(pair[1])?);
    }
    let path = PathBuf::from(std::ffi::OsString::from_vec(bytes));
    valid(&path).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visits_are_unique_ranked_and_bounded() {
        let mut history = History::default();
        history.visit("/old".into(), 1);
        history.visit("/old".into(), 2);
        history.visit("/récent avec espaces".into(), 1_000_000);
        history.visit("relative".into(), 1_000_000);
        assert_eq!(history.ranked(1_000_001)[0], PathBuf::from("/récent avec espaces"));
        assert_eq!(history.entries().len(), 2);
        assert_eq!(history.entries()[0].visits, 2);
        for i in 0..MAX_ENTRIES + 20 { history.visit(format!("/project-{i}").into(), 2_000_000 + i as u64); }
        assert_eq!(history.entries().len(), MAX_ENTRIES);
        assert!(!history.ranked(3_000_000).contains(&PathBuf::from("/old")));
    }
    #[test]
    fn restore_merges_duplicates_and_rejects_relative_paths() {
        let history = History::restore(vec![
            Visit { directory: "/a".into(), visits: 2, last_used: 10 },
            Visit { directory: "/a".into(), visits: 3, last_used: 20 },
            Visit { directory: "relative".into(), visits: 3, last_used: 20 },
        ]);
        assert_eq!(history.entries(), &[Visit { directory: "/a".into(), visits: 5, last_used: 20 }]);
    }
    #[test]
    fn cwd_event_validates_token_and_preserves_path_bytes() {
        let path = "/tmp/dossier é|\n";
        let encoded: String = path.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(parse_cwd(&format!("ATELIER_CWD:secret:{encoded}"), "secret"), Some(path.into()));
        for title in ["ATELIER_CWD:other:2f", "ATELIER_CWD:secret:zz", "ATELIER_CWD:secret:2f00", "ATELIER_CWD:secret:2fff", "ATELIER_CWD:secret:61", "ATELIER_CWD:secret:2"] {
            assert!(parse_cwd(title, "secret").is_none());
        }
    }
}
