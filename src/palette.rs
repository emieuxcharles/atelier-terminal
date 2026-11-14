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
        let response = egui::Modal::new(egui::Id::new("command-palette"))
            .frame(egui::Frame::new()
                .fill(egui::Color32::from_rgb(37, 39, 41))
                .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(58, 60, 63)))
                .corner_radius(8)
                .inner_margin(14.0))
            .show(ctx, |ui| {
            ui.set_width(540.0_f32.min((ctx.screen_rect().width() - 64.0).max(180.0)));
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(157, 158, 160));
            if title != "Palette de commandes" {
                ui.label(egui::RichText::new(title).size(14.0).color(egui::Color32::from_rgb(230, 230, 232)));
                if !subtitle.is_empty() { ui.label(subtitle); }
                ui.add_space(10.0);
            }
            let search = egui::Frame::new()
                .fill(egui::Color32::from_rgb(24, 26, 28))
                .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(112, 139, 172)))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(10, 7))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("⌘").size(17.0));
                        let search = ui.add_sized([ui.available_width() - 34.0, 20.0],
                            egui::TextEdit::singleline(&mut self.query)
                                .font(egui::FontId::proportional(13.0))
                                .text_color(egui::Color32::from_rgb(235, 235, 237))
                                .frame(false)
                                .hint_text(if title == "Docker Compose" { "Rechercher un service…" } else { "Rechercher une commande…" }));
                        ui.label(egui::RichText::new("Esc").size(11.0));
                        search
                    }).inner
                }).inner;
            ui.add_space(6.0);
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
            egui::ScrollArea::vertical().max_height((ctx.screen_rect().height() * 0.55).min(360.0)).show(ui, |ui| {
                let mut previous_group = "";
                for (index, (_, entry)) in matches.iter().enumerate() {
                    let group = match entry.target {
                        Target::Project(_) => "Projets",
                        Target::Pane(_) => "Panneaux",
                        Target::Directory(_) => "Dossiers récents",
                        Target::Compose(_) => "Docker Compose",
                        Target::Action(_) => "Commandes",
                    };
                    if group != previous_group {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(group).size(11.0).color(egui::Color32::from_rgb(122, 124, 127)));
                        ui.add_space(2.0);
                        previous_group = group;
                    }
                    let detail = !entry.detail.is_empty() && entry.detail != "Action";
                    let (rect, row) = ui.allocate_exact_size(egui::vec2(ui.available_width(), if detail { 42.0 } else { 28.0 }), egui::Sense::click());
                    let selected = index == self.selected;
                    if selected || row.hovered() {
                        ui.painter().rect_filled(rect, 4.0, if selected { egui::Color32::from_rgb(57, 59, 62) } else { egui::Color32::from_rgb(46, 48, 51) });
                    }
                    let mut right = rect.right() - 8.0;
                    let shortcut = entry.shortcut.replace('⌘', "cmd+").replace('⇧', "shift+").replace('⌥', "alt+");
                    for key in shortcut.split('+').rev().filter(|key| !key.is_empty()) {
                        let galley = ui.painter().layout_no_wrap(key.trim().to_owned(), egui::FontId::proportional(10.0), egui::Color32::from_rgb(145, 147, 150));
                        let badge = egui::Rect::from_min_size(egui::pos2(right - galley.size().x - 8.0, rect.center().y - 8.5), egui::vec2(galley.size().x + 8.0, 17.0));
                        ui.painter().rect(badge, 3.0, egui::Color32::from_rgb(42, 44, 46), egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(65, 67, 70)), egui::StrokeKind::Inside);
                        ui.painter().galley(badge.center() - galley.size() * 0.5, galley, egui::Color32::WHITE);
                        right = badge.left() - 4.0;
                    }
                    let text_rect = egui::Rect::from_min_max(rect.min + egui::vec2(8.0, 0.0), egui::pos2(right - 8.0, rect.bottom()));
                    let painter = ui.painter().with_clip_rect(text_rect);
                    painter.text(egui::pos2(text_rect.left(), rect.center().y - if detail { 7.0 } else { 0.0 }), egui::Align2::LEFT_CENTER, &entry.label, egui::FontId::proportional(13.0), if selected { egui::Color32::from_rgb(235, 235, 237) } else { egui::Color32::from_rgb(170, 172, 175) });
                    if detail {
                        painter.text(egui::pos2(text_rect.left(), rect.center().y + 9.0), egui::Align2::LEFT_CENTER, &entry.detail, egui::FontId::proportional(11.0), egui::Color32::from_rgb(125, 128, 132));
                    }
                    if (up || down || search.changed()) && selected { row.scroll_to_me(None); }
                    if row.clicked() { chosen = Some(entry.target.clone()); }
                }
            });
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
