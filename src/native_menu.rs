use eframe::egui;
use muda::{accelerator::{Accelerator, Code, Modifiers}, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use std::sync::mpsc;

#[derive(Clone, Copy)]
pub enum Action { Compose, Palette, OpenProject, CloseProject, ReopenProject, Split, Settings, ToggleSidebar, ToggleZoom, PreviousProject, NextProject, PreviousPane, NextPane }

pub struct NativeMenu {
    _menu: Menu,
    open: MenuItem,
    palette: MenuItem,
    close: MenuItem,
    reopen: MenuItem,
    split: MenuItem,
    compose: MenuItem,
    settings: MenuItem,
    sidebar: MenuItem,
    zoom: MenuItem,
    project_navigation: [MenuItem; 2],
    pane_navigation: [MenuItem; 2],
    actions: mpsc::Receiver<Action>,
}

impl NativeMenu {
    pub fn new(ctx: egui::Context) -> muda::Result<Self> {
        let palette = MenuItem::with_id("palette", "Palette de commandes…", true,
            Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyP)));
        let open = MenuItem::with_id("open-project", "Ouvrir un projet…", true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyO)));
        let close = MenuItem::with_id("close-project", "Fermer le projet", false,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyW)));
        let reopen = MenuItem::with_id("reopen-project", "Rouvrir le dernier projet fermé", false,
            Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyT)));
        let split = MenuItem::with_id("split", "Diviser le terminal", true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyD)));
        let settings = MenuItem::with_id("settings", "Réglages…", true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::Comma)));
        let sidebar = MenuItem::with_id("toggle-sidebar", "Masquer la barre des projets", true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyB)));
        let zoom = MenuItem::with_id("toggle-zoom", "Agrandir le panneau actif", true,
            Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::ALT), Code::Enter)));
        let project_navigation = [
            MenuItem::with_id("previous-project", "Projet précédent", true,
                Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::ALT), Code::ArrowUp))),
            MenuItem::with_id("next-project", "Projet suivant", true,
                Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::ALT), Code::ArrowDown))),
        ];
        let pane_navigation = [
            MenuItem::with_id("previous-pane", "Panneau précédent", true,
                Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::ALT), Code::ArrowLeft))),
            MenuItem::with_id("next-pane", "Panneau suivant", true,
                Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::ALT), Code::ArrowRight))),
        ];
        let app = Submenu::with_items("Atelier", true, &[
            &settings,
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(Some("Masquer Atelier")),
            &PredefinedMenuItem::hide_others(Some("Masquer les autres")),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(Some("Quitter Atelier")),
        ])?;
        let file = Submenu::with_items("Fichier", true, &[&open, &close, &reopen])?;
        let compose=MenuItem::with_id("docker-compose","Docker Compose…",true,None);
        let terminal = Submenu::with_items("Terminal", true, &[&split,&compose])?;
        let view = Submenu::with_items("Affichage", true, &[
            &palette, &PredefinedMenuItem::separator(), &sidebar, &zoom, &PredefinedMenuItem::separator(),
            &project_navigation[0], &project_navigation[1],
            &PredefinedMenuItem::separator(), &pane_navigation[0], &pane_navigation[1],
        ])?;
        let menu = Menu::with_items(&[&app, &file, &terminal, &view])?;
        let (sender, actions) = mpsc::channel();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let action = match event.id.0.as_str() {
                "docker-compose" => Action::Compose,
                "palette" => Action::Palette,
                "open-project" => Action::OpenProject,
                "close-project" => Action::CloseProject,
                "reopen-project" => Action::ReopenProject,
                "split" => Action::Split,
                "settings" => Action::Settings,
                "toggle-sidebar" => Action::ToggleSidebar,
                "toggle-zoom" => Action::ToggleZoom,
                "previous-project" => Action::PreviousProject,
                "next-project" => Action::NextProject,
                "previous-pane" => Action::PreviousPane,
                "next-pane" => Action::NextPane,
                _ => return,
            };
            if sender.send(action).is_ok() { ctx.request_repaint(); }
        }));
        menu.init_for_nsapp();
        Ok(Self { _menu: menu, palette, open, close, reopen, split, compose, settings, sidebar, zoom, project_navigation, pane_navigation, actions })
    }

    pub fn drain(&self) -> Vec<Action> { self.actions.try_iter().collect() }

    pub fn update(&self, can_reopen: bool, project_count: usize, pane_count: usize, dialog_open: bool, palette_open: bool, sidebar_visible: bool, zoomed: bool) {
        let has_project = project_count > 0;
        for item in &self.project_navigation { item.set_enabled(project_count > 1 && !dialog_open); }
        for item in &self.pane_navigation { item.set_enabled(pane_count > 1 && !dialog_open); }
        let sidebar_text = if sidebar_visible { "Masquer la barre des projets" } else { "Afficher la barre des projets" };
        let zoom_text = if zoomed { "Rétablir les panneaux" } else { "Agrandir le panneau actif" };
        if self.sidebar.text() != sidebar_text { self.sidebar.set_text(sidebar_text); }
        if self.zoom.text() != zoom_text { self.zoom.set_text(zoom_text); }
        self.sidebar.set_enabled(!dialog_open);
        self.zoom.set_enabled(has_project && !dialog_open);
        self.palette.set_enabled(!dialog_open || palette_open);
        self.open.set_enabled(!dialog_open);
        self.reopen.set_enabled(can_reopen && !dialog_open);
        self.close.set_enabled(has_project && !dialog_open);
        self.split.set_enabled(has_project && !dialog_open);
        self.compose.set_enabled(has_project && !dialog_open);
        self.settings.set_enabled(!dialog_open);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zoom_shortcut_uses_macbook_return_key() {
        let shortcut = Accelerator::new(Some(Modifiers::SUPER | Modifiers::ALT), Code::Enter);
        assert_eq!(shortcut.key_equivalent().unwrap(), "\r");
        let keypad = Accelerator::new(Some(Modifiers::SUPER | Modifiers::ALT), Code::NumpadEnter);
        assert_eq!(keypad.key_equivalent().unwrap(), "\u{0003}");
    }
}
