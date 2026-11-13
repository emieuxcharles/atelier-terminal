use crate::native_menu::Action;
use eframe::egui;

#[derive(Clone)]
pub enum Target { Action(Action), Pane(u64), Project(u64), Directory(std::path::PathBuf), Compose(crate::compose::Action) }

pub struct Entry {
    pub label: String,
    pub detail: String,
    pub shortcut: &'static str,
    pub target: Target,
}

#[derive(Default)]
pub struct Palette {
    pub open: bool,
    query: String,
    selected: usize,
}

fn normalized(value: &str) -> String {
    value.to_lowercase().chars().map(|c| match c {
        'é' | 'è' | 'ê' | 'ë' => 'e', 'à' | 'â' | 'ä' => 'a',
        'î' | 'ï' => 'i', 'ô' | 'ö' => 'o', 'ù' | 'û' | 'ü' => 'u', 'ç' => 'c', _ => c,
    }).collect()
}

fn score(query: &str, text: &str) -> Option<usize> {
    let query = normalized(query);
    let text = normalized(text);
    if query.trim().is_empty() { return Some(0); }
    let mut total = 0;
    for word in query.split_whitespace() {
        if let Some(position) = text.find(word) { total += position; continue; }
        let mut chars = word.chars();
        let mut next = chars.next();
        let mut cost = 1000;
        for (index, c) in text.chars().enumerate() {
            if Some(c) == next { cost += index; next = chars.next(); }
            if next.is_none() { break; }
        }
        if next.is_some() { return None; }
        total += cost;
    }
    Some(total)
}

impl Palette {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.query.clear();
        self.selected = 0;
    }

    pub fn reset_open(&mut self) { self.open=true; self.query.clear(); self.selected=0; }
    pub fn show(&mut self, ctx: &egui::Context, entries: &[Entry]) -> Option<Target> {
        self.show_named(ctx, entries, "Palette de commandes", "")
    }
    pub fn show_named(&mut self, ctx: &egui::Context, entries: &[Entry], title:&str, subtitle:&str) -> Option<Target> {
        let mut chosen = None;
        let escape = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let enter = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        let up = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
        let down = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
        let response = egui::Modal::new(egui::Id::new("command-palette")).show(ctx, |ui| {
            ui.set_width(560.0);
            ui.heading(title);
            if !subtitle.is_empty() {ui.label(subtitle);}
            let search = ui.add(egui::TextEdit::singleline(&mut self.query)
                .hint_text(if title == "Docker Compose" {"Rechercher un service ou une action Docker…"} else {"Rechercher un projet, un panneau ou une action…"}).desired_width(f32::INFINITY));
            search.request_focus();
            if search.changed() { self.selected = 0; }
            let mut matches: Vec<_> = entries.iter().filter_map(|entry| {
                score(&self.query, &format!("{} {}", entry.label, entry.detail)).map(|score| (score, entry))
            }).collect();
            matches.sort_by_key(|(score, entry)| (match entry.target { Target::Project(_) => 0, Target::Directory(_) => 2, _ => 1 }, *score));
            matches.truncate(100);
            self.selected = self.selected.min(matches.len().saturating_sub(1));
            if !matches.is_empty() {
                if down { self.selected = (self.selected + 1) % matches.len(); }
                if up { self.selected = (self.selected + matches.len() - 1) % matches.len(); }
                if enter { chosen = Some(matches[self.selected].1.target.clone()); }
            } else { ui.label("Aucun résultat."); }
            egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                for (index, (_, entry)) in matches.iter().enumerate() {
                    let text = format!("{}    {}\n{}", entry.label, entry.shortcut, entry.detail);
                    let row = ui.add_sized([ui.available_width(), 46.0], egui::Button::new(text).selected(index == self.selected));
                    if (up || down || search.changed()) && index == self.selected { row.scroll_to_me(None); }
                    if row.clicked() { chosen = Some(entry.target.clone()); }
                }
            });
            ui.separator();
            ui.weak("Flèches : choisir · Entrée : exécuter · Échap : fermer");
        });
        if escape || response.backdrop_response.clicked() || chosen.is_some() { self.open = false; }
        chosen
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(palette: &mut Palette, ctx: &egui::Context, entries: &[Entry], key: Option<egui::Key>) -> Option<Target> {
        let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0))), ..Default::default() };
        if let Some(key) = key {
            input.events.push(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
        }
        let mut result = None;
        let _ = ctx.run(input, |ctx| { if let Some(target) = palette.show(ctx, entries) { result = Some(target); } });
        result
    }

    #[test]
    fn keyboard_selection_execution_and_escape() {
        let ctx = egui::Context::default();
        let mut palette = Palette::default();
        palette.toggle();
        let entries: Vec<_> = (1..=2).map(|id| Entry { label: format!("Panneau {id}"), detail: "Projet test".into(), shortcut: "", target: Target::Pane(id) }).collect();
        frame(&mut palette, &ctx, &entries, None);
        frame(&mut palette, &ctx, &entries, Some(egui::Key::ArrowDown));
        assert!(matches!(frame(&mut palette, &ctx, &entries, Some(egui::Key::Enter)), Some(Target::Pane(2))));
        assert!(!palette.open);
        palette.toggle();
        assert!(frame(&mut palette, &ctx, &entries, Some(egui::Key::Escape)).is_none());
        assert!(!palette.open);
        palette.toggle();
        palette.query = "introuvable".into();
        assert!(frame(&mut palette, &ctx, &entries, Some(egui::Key::Enter)).is_none());
        assert!(palette.open);
    }
    #[test]
    fn opened_projects_win_and_history_opens_by_keyboard() {
        let ctx = egui::Context::default();
        let mut palette = Palette::default();
        let entries = vec![
            Entry { label: "magic".into(), detail: "".into(), shortcut: "", target: Target::Directory("/magic".into()) },
            Entry { label: "Projet Magic Evasion".into(), detail: "".into(), shortcut: "", target: Target::Project(7) },
        ];
        palette.toggle();
        palette.query = "magic".into();
        frame(&mut palette, &ctx, &entries, None);
        assert!(matches!(frame(&mut palette, &ctx, &entries, Some(egui::Key::Enter)), Some(Target::Project(7))));
        palette.toggle();
        palette.query = "magic".into();
        frame(&mut palette, &ctx, &entries, None);
        frame(&mut palette, &ctx, &entries, Some(egui::Key::ArrowDown));
        assert!(matches!(frame(&mut palette, &ctx, &entries, Some(egui::Key::Enter)), Some(Target::Directory(path)) if path == std::path::PathBuf::from("/magic")));
    }

    #[test]
    fn search_matches_accents_case_words_and_abbreviations() {
        assert!(score("REGLAGES", "Réglages de l’application").is_some());
        assert!(score("mag eva", "Projet Magic Evasion").is_some());
        assert!(score("mgev", "Magic Evasion").is_some());
        assert!(score("docker", "Réglages").is_none());
        assert!(score("projet", "Projet test").unwrap() < score("projet", "Prochaine journée été").unwrap());
        assert_eq!(score("", "Projet"), Some(0));
    }
}
