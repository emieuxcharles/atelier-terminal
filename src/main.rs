mod compose;
#[cfg(test)]
mod performance_tests;
mod directory_history;


mod appearance;
mod chrome;
mod completion;
mod persistence;
mod navigation;
mod palette;
mod native_menu;
mod working_directory;
mod ssh_status;

use eframe::egui::{self, FontId, RichText};
use egui_term::{BackendSettings, FontSettings, PtyEvent, TerminalBackend, TerminalFont, TerminalView};
use std::{path::PathBuf, sync::mpsc};

struct Pane {
    backend: TerminalBackend,
    title: String,
    exited: bool,
    ssh_host: Option<String>,
    docker_badge: Option<compose::Badge>,
    ssh_token: String,
    directory: PathBuf,
}

impl Pane {
    fn refresh_directory(&mut self) {
        if !self.exited {
            if let Some(path) = working_directory::of_process(self.backend.shell_pid).filter(|path| path.to_str().is_some()) {
                self.directory = path;
            }
        }
    }
}

struct Project {
    directory: PathBuf,
    panes: Vec<Pane>,
    focused: usize,
    zoomed: bool,
}

impl Project {
    fn close_finished_splits(&mut self) {
        let focused_id = self.panes.get(self.focused).map(|pane| pane.backend.id);
        // Keep one pane even when several shells exit in the same event batch.
        for index in (0..self.panes.len()).rev() {
            if self.panes.len() > 1 && self.panes[index].exited {
                self.panes.remove(index);
            }
        }
        self.focused = self.panes.iter()
            .position(|pane| Some(pane.backend.id) == focused_id)
            .unwrap_or_else(|| self.focused.min(self.panes.len().saturating_sub(1)));
    }
}

struct Atelier {
    appearances: [std::sync::Arc<appearance::Appearance>; 2],
    styled_dark: Option<bool>,
    directory_history: directory_history::History,
    palette_directories: Vec<PathBuf>,
    projects: Vec<Project>,
    closed_projects: Vec<persistence::SavedProject>,
    selected: usize,
    next_id: u64,
    ssh_integration: Option<ssh_status::Integration>,
    sender: mpsc::Sender<(u64, PtyEvent)>,
    receiver: mpsc::Receiver<(u64, PtyEvent)>,
    directory: String,
    completion: completion::Completion,
    adding: bool,
    settings_open: bool,
    palette: palette::Palette,
    compose: compose::Browser,
    sidebar_visible: bool,
    native_menu: Option<native_menu::NativeMenu>,
    error: Option<String>,
    font_size: f32,
    context: egui::Context,
    state_path: PathBuf,
    last_saved: Option<persistence::State>,
    save_enabled: bool,
    save_error: Option<String>,
}

impl Atelier {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_theme(egui::ThemePreference::System);
        let (sender, receiver) = mpsc::channel();
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let mut app = Self {
            appearances: [std::sync::Arc::new(appearance::Appearance::for_visuals(&egui::Visuals::light())), std::sync::Arc::new(appearance::Appearance::for_visuals(&egui::Visuals::dark()))], styled_dark: None,
            directory_history: directory_history::History::default(), palette_directories: vec![], projects: vec![], closed_projects: vec![], selected: 0, next_id: 0, ssh_integration: None, sender, receiver,
            directory: home.clone(), completion: completion::Completion::new(cc.egui_ctx.clone()), adding: false, settings_open: false, palette: palette::Palette::default(), compose: compose::Browser::default(), sidebar_visible: true, native_menu: None, error: None, font_size: 15.0,
            context: cc.egui_ctx.clone(),
            state_path: persistence::path(), last_saved: None, save_enabled: true, save_error: None,
        };
        match ssh_status::Integration::install() {
            Ok(integration) => app.ssh_integration = Some(integration),
            Err(err) => app.error = Some(format!("Repère SSH indisponible : {err}")),
        }
        match persistence::load(&app.state_path) {
            Ok(Some(saved)) => {
                app.compose.file_selections=saved.compose_files.clone();
                app.directory_history = directory_history::History::restore(saved.directory_history.clone());
                // Upgrade old state files from known projects without increasing existing counts.
                for project in saved.projects.iter().chain(&saved.closed_projects) {
                    app.directory_history.seed(project.directory.clone(), directory_history::now());
                    for path in &project.pane_directories { app.directory_history.seed(path.clone(), directory_history::now()); }
                }
                app.closed_projects = saved.closed_projects.clone();
                app.font_size = saved.font_size;
                app.sidebar_visible = saved.sidebar_visible;
                cc.egui_ctx.set_theme(match saved.theme {
                    persistence::Theme::System => egui::ThemePreference::System,
                    persistence::Theme::Light => egui::ThemePreference::Light,
                    persistence::Theme::Dark => egui::ThemePreference::Dark,
                });
                let mut selected = 0;
                let mut unavailable = Vec::new();
                for (index, project) in saved.projects.iter().enumerate() {
                    let before = app.projects.len();
                    match app.restore_project(&cc.egui_ctx, project) {
                        Ok(restored) => { app.projects.push(restored); app.selected = app.projects.len() - 1; }
                        Err(err) => app.error = Some(err.to_string()),
                    }
                    if app.projects.len() == before {
                        unavailable.push(project.directory.display().to_string());
                        continue;
                    }
                    if index == saved.selected { selected = app.selected; }
                    let restored = app.projects.last_mut().unwrap();
                    restored.focused = project.focused.min(restored.panes.len() - 1);
                }
                app.selected = selected;
                if !unavailable.is_empty() {
                    app.error = Some(format!("Projets indisponibles : {}", unavailable.join(", ")));
                }
                // Do not rewrite the file just because a directory is temporarily unavailable.
                app.last_saved = Some(app.snapshot(&cc.egui_ctx));
            }
            Ok(None) => app.add_project(&cc.egui_ctx, PathBuf::from(home)),
            Err(err) => {
                app.add_project(&cc.egui_ctx, PathBuf::from(home));
                app.save_enabled = false;
                app.save_error = Some(format!("Sauvegarde illisible, fichier préservé : {err}"));
            }
        }
        match native_menu::NativeMenu::new(cc.egui_ctx.clone()) {
            Ok(menu) => app.native_menu = Some(menu),
            Err(err) => app.error = Some(format!("Menu macOS indisponible : {err}")),
        }
        app
    }

    // Workspace-only snapshot for cheap per-frame change detection; persist adds history on writes.
    fn snapshot(&self, ctx: &egui::Context) -> persistence::State {
        persistence::State {
            compose_files:self.compose.file_selections.clone(),
            projects: self.projects.iter().map(|project| persistence::SavedProject {
                directory: project.directory.clone(), panes: project.panes.len(), focused: project.focused,
                pane_directories: project.panes.iter().map(|pane| pane.directory.clone()).collect(),
            }).collect(),
            closed_projects: self.closed_projects.clone(),
            selected: self.selected,
            font_size: self.font_size,
            sidebar_visible: self.sidebar_visible,
            theme: match ctx.options(|options| options.theme_preference) {
                egui::ThemePreference::System => persistence::Theme::System,
                egui::ThemePreference::Light => persistence::Theme::Light,
                egui::ThemePreference::Dark => persistence::Theme::Dark,
            },
            ..persistence::State::default()
        }
    }

    fn persist(&mut self, ctx: &egui::Context) {
        if !self.save_enabled { return; }
        let mut state = self.snapshot(ctx);
        if self.last_saved.as_ref() == Some(&state) && !self.directory_history.dirty { return; }
        // Clone the bounded history only for an actual write, never for terminal redraws.
        state.directory_history = self.directory_history.entries().to_vec();
        match persistence::save(&self.state_path, &state) {
            Ok(()) => { state.directory_history.clear(); self.last_saved = Some(state); self.directory_history.dirty = false; self.save_error = None; }
            Err(err) => {
                self.save_error = Some(format!("Échec de sauvegarde : {err}"));
                // Avoid retrying on every terminal output event.
                self.save_enabled = false;
            }
        }
    }

    fn close_project(&mut self, index: usize) {
        if index >= self.projects.len() { return; }
        let mut project = self.projects.remove(index);
        for pane in &mut project.panes { pane.refresh_directory(); }
        persistence::remember_closed(&mut self.closed_projects, persistence::SavedProject {
            directory: project.directory.clone(), panes: project.panes.len(), focused: project.focused,
                pane_directories: project.panes.iter().map(|pane| pane.directory.clone()).collect(),
        });
        // Dropping the project shuts down its PTYs; history retains only metadata.
        drop(project);
        if index < self.selected { self.selected -= 1; }
        self.selected = self.selected.min(self.projects.len().saturating_sub(1));
    }

    fn reopen_project(&mut self, ctx: &egui::Context) {
        let Some(saved) = self.closed_projects.last().cloned() else { return; };
        if !saved.directory.is_dir() {
            self.error = Some(format!("Impossible de rouvrir ce dossier : {}. L’entrée reste dans l’historique.", saved.directory.display()));
            return;
        }
        match self.restore_project(ctx, &saved) {
            Ok(project) => self.projects.push(project),
            Err(err) => { self.error = Some(format!("Réouverture impossible : {err}. Historique conservé.")); return; }
        }
        self.directory_history.visit(saved.directory.clone(), directory_history::now());
        self.closed_projects.pop();
        self.selected = self.projects.len() - 1;
        self.error = None;
    }

    fn restore_project(&mut self, ctx: &egui::Context, saved: &persistence::SavedProject) -> std::io::Result<Project> {
        let mut panes = Vec::with_capacity(saved.panes);
        for index in 0..saved.panes {
            let directory = saved.pane_directory(index);
            // A deleted subdirectory must not prevent restoring the rest of the project.
            let directory = if directory.is_dir() { directory } else { saved.directory.clone() };
            panes.push(self.pane(ctx, directory)?);
        }
        Ok(Project { directory: saved.directory.clone(), focused: saved.focused.min(panes.len().saturating_sub(1)), panes, zoomed: false })
    }

    fn execute_action(&mut self, ctx: &egui::Context, action: native_menu::Action) {
        match action {
                native_menu::Action::Compose if !self.adding && !self.settings_open => self.compose_action(ctx,compose::Action::Open),
                native_menu::Action::Palette if !self.adding && !self.settings_open => self.toggle_palette(ctx),
                native_menu::Action::OpenProject if !self.settings_open => self.adding = true,
                native_menu::Action::CloseProject if !self.adding && !self.settings_open => self.close_project(self.selected),
                native_menu::Action::ReopenProject if !self.adding && !self.settings_open => self.reopen_project(ctx),
                native_menu::Action::Split if !self.adding && !self.settings_open => self.split(ctx),
                native_menu::Action::Settings if !self.adding => self.settings_open = true,
                native_menu::Action::ToggleSidebar if !self.adding && !self.settings_open => {
                    self.sidebar_visible = !self.sidebar_visible;
                }
                native_menu::Action::ToggleZoom if !self.adding && !self.settings_open => {
                    if let Some(project) = self.projects.get_mut(self.selected) {
                        project.zoomed = !project.zoomed;
                    }
                }
                native_menu::Action::PreviousProject | native_menu::Action::NextProject
                    if !self.adding && !self.settings_open => {
                    self.selected = navigation::adjacent(self.selected, self.projects.len(),
                        matches!(action, native_menu::Action::NextProject));
                }
                native_menu::Action::PreviousPane | native_menu::Action::NextPane
                    if !self.adding && !self.settings_open => {
                    if let Some(project) = self.projects.get_mut(self.selected) {
                        project.focused = navigation::adjacent(project.focused, project.panes.len(),
                            matches!(action, native_menu::Action::NextPane));
                    }
                }
                _ => {}
            }
    }

    fn compose_action(&mut self,ctx:&egui::Context,action:compose::Action) {
        use compose::Action;
        match action {
            Action::Open=>{if let Some(project)=self.projects.get(self.selected){self.compose.open(project.directory.clone(),ctx.clone());self.palette.reset_open();}},
            Action::Back=>{self.compose.leave();self.palette.reset_open();},
            Action::Refresh=>{self.compose.refresh(ctx.clone());self.palette.reset_open();},
            Action::Configure=>{self.compose.configure=true;},
            Action::Up|Action::Stop|Action::Restart(_)=>{self.compose.mutate(action,ctx.clone());self.palette.reset_open();},
            Action::Shell(_)|Action::Logs(_)=>{
                let Some(snapshot)=self.compose.snapshot.as_ref() else{return};
                let binding=snapshot.binding.clone();
                let Some(project_index)=self.projects.iter().position(|p|p.directory==binding.directory) else{return};
                let Some((args,badge))=compose::terminal_args(&binding,&action) else{return};
                self.next_id+=1;let token=ssh_status::token();
                let mut env=self.ssh_integration.as_ref().map(|i|i.environment(&token,true)).unwrap_or_default();
                env.insert("ATELIER_SSH_TOKEN".into(),token.clone());
                // Arguments remain separate; neither filenames nor service names enter shell source.
                let script=compose::RETURN_TO_LOCAL_SCRIPT;
                let mut shell_args=vec!["-f".into(),"-c".into(),script.into(),"atelier-compose".into(),binding.docker.to_string_lossy().into_owned()];shell_args.extend(args);
                match TerminalBackend::new(self.next_id,ctx.clone(),self.sender.clone(),BackendSettings{shell:"/bin/zsh".into(),args:shell_args,working_directory:Some(binding.directory.clone()),env}) {
                    Ok(backend)=>{let project=&mut self.projects[project_index];project.panes.push(Pane{backend,title:if matches!(action,Action::Shell(_)){"Shell".into()}else{"Logs".into()},exited:false,ssh_host:None,docker_badge:Some(badge),ssh_token:token,directory:binding.directory});project.focused=project.panes.len()-1;self.selected=project_index;},
                    Err(e)=>self.error=Some(format!("Impossible d’ouvrir Docker : {e}")),
                }
                self.compose.leave();
            }
        }
    }

    fn toggle_palette(&mut self, ctx: &egui::Context) {
        self.compose.leave();
        self.palette.toggle();
        if self.palette.open {
            // Explicit-action fallback for shells other than zsh. No polling or scanning.
            for project in &mut self.projects {
                for pane in &mut project.panes {
                    let before = pane.directory.clone();
                    pane.refresh_directory();
                    if before != pane.directory { self.directory_history.visit(pane.directory.clone(), directory_history::now()); }
                }
            }
            self.palette_directories = self.directory_history.ranked(directory_history::now());
        } else { self.palette_directories.clear(); }
        ctx.request_repaint();
    }

    fn palette_entries(&self) -> Vec<palette::Entry> {
        use native_menu::Action::*;
        if self.compose.active {return self.compose.entries();}
        let mut entries = Vec::new();
        if !self.projects.is_empty() {entries.push(palette::Entry {label:"Docker Compose…".into(),detail:"Services et actions du projet · interrogation à la demande".into(),shortcut:"",target:palette::Target::Compose(compose::Action::Open)});}
        let mut project_paths = std::collections::HashSet::new();
        for project in &self.projects {
            let name = project.directory.file_name().unwrap_or_default().to_string_lossy();
            let path = project.directory.display().to_string();
            if let Some(pane) = project.panes.get(project.focused).filter(|_| project_paths.insert(project.directory.clone())) {
                entries.push(palette::Entry { label: format!("Projet : {name}"), detail: path.clone(), shortcut: "",
                    target: palette::Target::Project(pane.backend.id) });
            }
            for (index, pane) in project.panes.iter().enumerate() {
                entries.push(palette::Entry { label: format!("Panneau {} · {}", index + 1, name),
                    detail: format!("{} · {}", path, pane.title), shortcut: "",
                    target: palette::Target::Pane(pane.backend.id) });
            }
        }
        for directory in &self.palette_directories {
            if self.projects.iter().any(|p| p.directory == *directory) { continue; }
            let name = directory.file_name().unwrap_or_default().to_string_lossy();
            entries.push(palette::Entry { label: format!("Ouvrir : {name}"),
                detail: format!("Dossier visité · {}", directory.display()), shortcut: "",
                target: palette::Target::Directory(directory.clone()) });
        }
        let has_project = !self.projects.is_empty();
        let has_panes = self.projects.get(self.selected).is_some_and(|p| p.panes.len() > 1);
        let zoomed = self.projects.get(self.selected).is_some_and(|p| p.zoomed);
        let actions = [
            ("Ouvrir un projet…", "⌘O", OpenProject, true),
            ("Fermer le projet actif", "⌘W", CloseProject, has_project),
            ("Rouvrir le dernier projet fermé", "⌘⇧T", ReopenProject, !self.closed_projects.is_empty()),
            ("Diviser le terminal", "⌘D", Split, has_project),
            ("Réglages…", "⌘,", Settings, true),
            (if self.sidebar_visible { "Masquer la barre des projets" } else { "Afficher la barre des projets" }, "⌘B", ToggleSidebar, true),
            (if zoomed { "Rétablir les panneaux" } else { "Agrandir le panneau actif" }, "⌥⌘Entrée", ToggleZoom, has_project),
            ("Projet précédent", "⌥⌘↑", PreviousProject, self.projects.len() > 1),
            ("Projet suivant", "⌥⌘↓", NextProject, self.projects.len() > 1),
            ("Panneau précédent", "⌥⌘←", PreviousPane, has_panes),
            ("Panneau suivant", "⌥⌘→", NextPane, has_panes),
        ];
        for (label, shortcut, action, enabled) in actions {
            if enabled { entries.push(palette::Entry { label: label.into(), detail: "Action".into(), shortcut,
                target: palette::Target::Action(action) }); }
        }
        entries
    }

    fn pane(&mut self, ctx: &egui::Context, directory: PathBuf) -> std::io::Result<Pane> {
        self.next_id += 1;
        let ssh_token = ssh_status::token();
        let env = self.ssh_integration.as_ref().map(|integration| integration.environment(&ssh_token, std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into()).ends_with("zsh"))).unwrap_or_default();
        let backend = TerminalBackend::new(self.next_id, ctx.clone(), self.sender.clone(), BackendSettings {
            shell: std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into()),
            env,
            args: vec!["-l".into()], working_directory: Some(directory.clone()),
        })?;
        Ok(Pane { backend, title: "Shell".into(), exited: false, directory, ssh_host: None, docker_badge: None, ssh_token })
    }

    fn add_project(&mut self, ctx: &egui::Context, directory: PathBuf) {
        let directory = match directory.canonicalize() {
            Ok(path) if path.is_dir() && path.to_str().is_some() => path,
            _ => { self.error = Some("Ce dossier n’existe pas ou n’est pas accessible.".into()); return; }
        };
        match self.pane(ctx, directory.clone()) {
            Ok(pane) => {
                self.directory_history.visit(directory.clone(), directory_history::now());
                self.projects.push(Project { directory, panes: vec![pane], focused: 0, zoomed: false });
                self.selected = self.projects.len() - 1;
                self.adding = false;
                self.error = None;
            }
            Err(err) => self.error = Some(format!("Impossible de démarrer le shell : {err}")),
        }
    }

    fn split(&mut self, ctx: &egui::Context) {
        let Some(project) = self.projects.get_mut(self.selected) else { return; };
        let Some(active) = project.panes.get_mut(project.focused) else { return; };
        active.refresh_directory();
        let directory = active.directory.clone();
        match self.pane(ctx, directory) {
            Ok(pane) => {
                let project = &mut self.projects[self.selected];
                project.panes.push(pane);
                project.focused = project.panes.len() - 1;
                project.zoomed = false;
            }
            Err(err) => self.error = Some(err.to_string()),
        }
    }
}

impl eframe::App for Atelier {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        for project in &mut self.projects {
            for pane in &mut project.panes { pane.refresh_directory(); }
        }
        let ctx = self.context.clone();
        self.persist(&ctx);
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.compose.poll();
        let menu_actions = self.native_menu.as_ref().map(|menu| menu.drain()).unwrap_or_default();
        for action in menu_actions {
            if !self.palette.open || matches!(action, native_menu::Action::Palette) {
                self.execute_action(ctx, action);
            }
        }
        let dark = ctx.style().visuals.dark_mode;
        let appearance = self.appearances[dark as usize].clone();
        if self.styled_dark != Some(dark) { appearance.apply_style(ctx); self.styled_dark = Some(dark); }
        let visuals = ctx.style().visuals.clone();
        // PTY events wake egui; there is no continuous rendering timer.
        for _ in 0..512 {
            let Ok((id, event)) = self.receiver.try_recv() else { break; };
            for project in &mut self.projects {
                for pane in &mut project.panes {
                    if pane.backend.id == id {
                        match &event {
                            PtyEvent::Exit => { pane.exited = true; pane.ssh_host = None; pane.docker_badge=None; },
                            PtyEvent::Title(title) => {
                                if title == &format!("ATELIER_DOCKER:{}:local",pane.ssh_token) {
                                    pane.docker_badge=None;pane.ssh_host=None;pane.title="Shell".into();
                                } else if let Some(badge) = compose::parse_relay(title, &pane.ssh_token) {
                                    pane.docker_badge=Some(badge);
                                } else if let Some(path) = directory_history::parse_cwd(title, &pane.ssh_token) {
                                    if path != pane.directory {
                                        self.directory_history.visit(path.clone(), directory_history::now());
                                        pane.directory = path;
                                    }
                                } else if let Some(host) = ssh_status::parse(title, &pane.ssh_token) {
                                    pane.ssh_host = host;
                                } else if !title.starts_with("ATELIER_CWD:") {
                                    pane.title = title.chars().take(80).collect();
                                }
                            },
                            PtyEvent::ResetTitle => pane.title = "Shell".into(),
                            _ => {}
                        }
                    }
                }
            }
        }
        for project in &mut self.projects {
            project.close_finished_splits();
        }
        if !self.settings_open && !self.adding && !self.palette.open && !self.compose.configure {
            let modifiers = egui::Modifiers { alt: true, ..egui::Modifiers::COMMAND };
            if self.native_menu.is_none() && ctx.input_mut(|i| i.consume_key(modifiers, egui::Key::B)) {
                self.sidebar_visible = !self.sidebar_visible;
            }
            let up = self.native_menu.is_none() && ctx.input_mut(|i| i.consume_key(modifiers, egui::Key::ArrowUp));
            let down = self.native_menu.is_none() && ctx.input_mut(|i| i.consume_key(modifiers, egui::Key::ArrowDown));
            let left = self.native_menu.is_none() && ctx.input_mut(|i| i.consume_key(modifiers, egui::Key::ArrowLeft));
            let right = self.native_menu.is_none() && ctx.input_mut(|i| i.consume_key(modifiers, egui::Key::ArrowRight));
            let zoom = self.native_menu.is_none() && ctx.input_mut(|i| i.consume_key(modifiers, egui::Key::Enter));
            if up || down {
                self.selected = navigation::adjacent(self.selected, self.projects.len(), down);
            }
            if let Some(project) = self.projects.get_mut(self.selected) {
                if left || right {
                    project.focused = navigation::adjacent(project.focused, project.panes.len(), right);
                }
                if zoom { project.zoomed = !project.zoomed; }
            }
        }
        if self.native_menu.is_none() && !self.palette.open && !self.compose.configure && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)) {
            self.settings_open = true;
        }
        if self.settings_open && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.settings_open = false;
        }
        if self.native_menu.is_none() && !self.palette.open && !self.settings_open && !self.adding && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::D)) {
            self.split(ctx);
        }
        if self.native_menu.is_none() && !self.palette.open && !self.settings_open && !self.adding && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.adding = true;
        }

        let blocked = self.adding || self.settings_open || self.palette.open || self.compose.configure;
        let mut toolbar_action = None;
        egui::TopBottomPanel::top("dense-toolbar").exact_height(40.0)
            .frame(egui::Frame::NONE.fill(appearance.sidebar).inner_margin(egui::Margin::symmetric(8, 4)))
            .show(ctx, |ui| {
                let drag = ui.interact(ui.max_rect(), ui.id().with("window-drag"), egui::Sense::drag());
                if drag.drag_started() { ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag); }
                ui.horizontal_centered(|ui| {
                    // Keep the native macOS close/minimize/fullscreen controls unobstructed.
                    if cfg!(target_os = "macos") { ui.add_space(70.0); }
                    ui.add_enabled_ui(!blocked, |ui| {
                        if chrome::button(ui, chrome::Icon::Sidebar, "Afficher/masquer les projets · ⌘B", egui::vec2(28.0,28.0)).clicked() { toolbar_action=Some(native_menu::Action::ToggleSidebar); }
                    });
                    let name = self.projects.get(self.selected).map(|p| p.directory.display().to_string()).unwrap_or_else(|| "Rechercher un projet…".into());
                    let width = (ui.available_width() - 104.0).max(80.0);
                    ui.add_enabled_ui(!self.adding && !self.settings_open, |ui| {
                        if ui.add_sized([width,28.0],egui::Button::new(RichText::new(name).size(12.0)).frame(false).truncate())
                            .on_hover_text("Palette de commandes · ⌘⇧P").clicked() { toolbar_action=Some(native_menu::Action::Palette); }
                    });
                    ui.add_enabled_ui(!blocked, |ui| {
                        if chrome::button(ui,chrome::Icon::Plus,"Ouvrir un projet · ⌘O",egui::vec2(26.0,28.0)).clicked() { toolbar_action=Some(native_menu::Action::OpenProject); }
                        ui.add_enabled_ui(!self.projects.is_empty(), |ui| {
                            if chrome::button(ui,chrome::Icon::Split,"Diviser · ⌘D",egui::vec2(26.0,28.0)).clicked() { toolbar_action=Some(native_menu::Action::Split); }
                            if chrome::button(ui,chrome::Icon::Expand,"Agrandir/rétablir le panneau actif · ⌥⌘Entrée",egui::vec2(26.0,28.0)).clicked() { toolbar_action=Some(native_menu::Action::ToggleZoom); }
                        });
                    });
                });
            });
        if let Some(action)=toolbar_action { self.execute_action(ctx,action); }
        let mut close_project = None;
        if self.sidebar_visible {
            egui::SidePanel::left("dense-projects").exact_width(151.0).resizable(false)
                .frame(egui::Frame::NONE.fill(appearance.sidebar).inner_margin(egui::Margin::symmetric(5,8)))
                .show(ctx, |ui| {
                    ui.add_space(2.0);
                    ui.label(RichText::new("Projets ouverts").size(11.0).color(appearance.muted));
                    ui.add_space(4.0);
                    ui.add_enabled_ui(!blocked, |ui| {
                        egui::ScrollArea::vertical().auto_shrink([false,false]).max_height((ui.available_height()-35.0).max(1.0)).show(ui, |ui| {
                            for (index,project) in self.projects.iter().enumerate() {
                                let name=project.directory.file_name().unwrap_or(project.directory.as_os_str()).to_string_lossy();
                                egui::Frame::NONE.fill(if index==self.selected {appearance.selected} else {egui::Color32::TRANSPARENT}).corner_radius(5.0).show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let (icon_rect,_)=ui.allocate_exact_size(egui::vec2(18.0,28.0),egui::Sense::hover());
                                        chrome::draw(ui,icon_rect,chrome::Icon::Folder,appearance.muted);
                                        let width=(ui.available_width()-26.0).max(20.0);
                                        if ui.add_sized([width,28.0],egui::Button::new(RichText::new(name).size(12.0)).frame(false).truncate())
                                            .on_hover_text(project.directory.display().to_string()).clicked() { self.selected=index; }
                                        if chrome::button(ui,chrome::Icon::Close,"Fermer le projet et ses shells · ⌘W",egui::vec2(20.0,28.0)).clicked() { close_project=Some(index); }
                                    });
                                });
                            }
                        });
                    });
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{} projets",self.projects.len())).size(11.0).color(appearance.muted));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_enabled_ui(!blocked, |ui| {
                                    if chrome::button(ui,chrome::Icon::Plus,"Ouvrir un projet · ⌘O",egui::vec2(24.0,24.0)).clicked() { self.adding=true; }
                                });
                            });
                        });
                    });
                });
        }
        if let Some(index)=close_project { self.close_project(index); }
        if let Some(error)=self.save_error.as_ref().or(self.error.as_ref()) {
            egui::TopBottomPanel::bottom("error-status").show(ctx,|ui| {ui.colored_label(visuals.error_fg_color,error);});
        }
        let mut restart = None;
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(appearance.background)).show(ctx, |ui| {
            let Some(project)=self.projects.get_mut(self.selected) else {
                ui.vertical_centered(|ui| { ui.add_space(80.0); ui.heading("Aucun projet ouvert"); if ui.button("Ouvrir un projet · ⌘O").clicked() { self.adding=true; } });
                return;
            };
            let rects=navigation::pane_rects(ui.max_rect(),project.panes.len(),project.focused,project.zoomed);
            // Resolve pointer focus before drawing any pane: no stale focus or one-frame mismatch.
            if !blocked {
                if let Some(point)=ctx.input(|i| i.pointer.any_pressed().then(||i.pointer.interact_pos()).flatten()) {
                    if let Some(index)=navigation::pane_at(&rects,point) { project.focused=index; }
                }
            }
            for (index,rect) in rects {
                let active=index==project.focused;
                let tint=|color| if active {color} else {(appearance.matte)(color)};
                let pane=&mut project.panes[index];
                ui.painter().rect_filled(rect,0.0,tint(appearance.background));
                let header=egui::Rect::from_min_size(rect.min,egui::vec2(rect.width(),26.0));
                ui.painter().rect_filled(header,0.0,tint(appearance.chrome));
                ui.scope_builder(egui::UiBuilder::new().id_salt(("pane-header",pane.backend.id)).max_rect(header.shrink2(egui::vec2(8.0,0.0))), |ui| {
                    ui.set_clip_rect(header);
                    ui.horizontal_centered(|ui| {
                        let machine_label=pane.docker_badge.as_ref().map(|badge|badge.machine.label());
                        let badge=machine_label.as_deref().unwrap_or(if pane.ssh_host.is_some() {"SSH"} else {"LOCAL"});
                        ui.spacing_mut().item_spacing.x=7.0;
                        let remote=pane.ssh_host.is_some() || pane.docker_badge.as_ref().is_some_and(|b|!matches!(b.machine,compose::Machine::Local));
                        let (badge_color,badge_background)=appearance.badge_colors(remote,active);
                        let compact=compose::compact_badge(badge);
                        let badge_rect=chrome::pane_badge(ui,&compact,badge_color,badge_background);
                        ui.interact(badge_rect,ui.id().with("machine-badge"),egui::Sense::hover()).on_hover_text(badge);
                        if let Some(container)=&pane.docker_badge {
                            let (fg,bg)=appearance.badge_colors(true,active);
                            let badge_rect=chrome::pane_badge(ui,&compose::compact_badge(&container.label),fg,bg);
                            ui.interact(badge_rect,ui.id().with("container-badge"),egui::Sense::hover()).on_hover_text(&container.label);
                        }
                        let title=pane.ssh_host.as_deref().unwrap_or(&pane.title);
                        let width=ui.available_width().max(0.0);
                        ui.allocate_ui_with_layout(egui::vec2(width,22.0),egui::Layout::left_to_right(egui::Align::Center),|ui| {
                            ui.add(egui::Label::new(RichText::new(title).size(11.0).color(tint(visuals.text_color()))).truncate())
                                .on_hover_text(pane.directory.display().to_string());
                        });
                    });
                });
                let terminal_rect=egui::Rect::from_min_max(header.left_bottom()+egui::vec2(10.0,8.0),rect.max-egui::vec2(10.0,6.0));
                if terminal_rect.is_positive() {
                    ui.scope_builder(egui::UiBuilder::new().id_salt(("terminal",pane.backend.id)).max_rect(terminal_rect),|ui| {
                        ui.set_clip_rect(terminal_rect);
                        if pane.exited {
                            ui.label(RichText::new("Session terminée.").color(tint(appearance.muted)));
                            if ui.button("Relancer le shell").clicked() { restart=Some(index); }
                        } else {
                            let view=TerminalView::new(ui,&mut pane.backend)
                                .set_font(TerminalFont::new(FontSettings {font_type:FontId::monospace(self.font_size)}))
                                .set_theme_ref(&appearance.terminal)
                                .set_color_transform(if active {None} else {Some(appearance.matte)})
                                .set_focus(active && !blocked);
                            ui.add(view);
                        }
                    });
                }
                ui.painter().line_segment([rect.right_top(),rect.right_bottom()],egui::Stroke::new(1.0_f32,appearance.separator));
                ui.painter().line_segment([rect.left_bottom(),rect.right_bottom()],egui::Stroke::new(1.0_f32,appearance.separator));
            }
        });
        if let Some(index) = restart {
            let directory = self.projects[self.selected].panes[index].directory.clone();
            match self.pane(ctx, directory) {
                Ok(pane) => self.projects[self.selected].panes[index] = pane,
                Err(err) => self.error = Some(err.to_string()),
            }
        }
        if self.settings_open {
            egui::Window::new("Réglages")
                .id(egui::Id::new("settings"))
                .open(&mut self.settings_open)
                .collapsible(false)
                .resizable(false)
                .default_width(420.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.heading("Apparence");
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.label("Thème");
                        let mut preference = ctx.options(|options| options.theme_preference);
                        let previous = preference;
                        egui::ComboBox::from_id_salt("appearance")
                            .selected_text(match preference {
                                egui::ThemePreference::System => "Système",
                                egui::ThemePreference::Light => "Clair",
                                egui::ThemePreference::Dark => "Sombre",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut preference, egui::ThemePreference::System, "Système");
                                ui.selectable_value(&mut preference, egui::ThemePreference::Light, "Clair");
                                ui.selectable_value(&mut preference, egui::ThemePreference::Dark, "Sombre");
                            });
                        if preference != previous { ctx.set_theme(preference); }
                    });
                    ui.label("Système suit l’apparence claire ou sombre de macOS.");
                    ui.add_space(16.0);
                    ui.label("Taille du texte du terminal");
                    ui.add(egui::Slider::new(&mut self.font_size, 11.0..=26.0).step_by(1.0).suffix(" pt"));
                    ui.add_space(16.0);
                    ui.separator();
                    ui.label("Les changements s’appliquent immédiatement.");
                    ui.weak("Réglages sauvegardés automatiquement. Échap pour fermer.");
                });
        }
        if self.adding {
            egui::Window::new("Ouvrir un projet").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0,0.0]).show(ctx, |ui| {
                ui.label("Dossier du projet (chemin absolu)");
                self.completion.update(&self.directory);
                let enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                let tab = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab));
                let up = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
                let down = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
                if !self.completion.suggestions.is_empty() {
                    let count = self.completion.suggestions.len();
                    if down { self.completion.selected = (self.completion.selected + 1) % count; }
                    if up { self.completion.selected = (self.completion.selected + count - 1) % count; }
                }
                let response = ui.add(egui::TextEdit::singleline(&mut self.directory).desired_width(500.0));
                response.request_focus();
                self.completion.update(&self.directory);
                let mut chosen = if tab { self.completion.suggestions.get(self.completion.selected).cloned() } else { None };
                ui.weak("↑ ↓ pour choisir · Tab pour compléter · Entrée pour ouvrir");
                if self.completion.loading { ui.label("Recherche des dossiers…"); }
                if let Some(error) = &self.completion.error { ui.weak(error); }
                egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                    for (index, path) in self.completion.suggestions.iter().enumerate() {
                        let item = ui.selectable_label(index == self.completion.selected, path);
                        if (up || down) && index == self.completion.selected { item.scroll_to_me(None); }
                        if item.clicked() { chosen = Some(path.clone()); }
                    }
                });
                if let Some(path) = chosen {
                    self.directory = path;
                    self.completion.update(&self.directory);
                    if let Some(mut state) = egui::TextEdit::load_state(ctx, response.id) {
                        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(self.directory.chars().count()))));
                        state.store(ctx, response.id);
                    }
                }
                ui.horizontal(|ui| {
                    if ui.button("Ouvrir").clicked() || enter {
                        let path = completion::expand_path(&self.directory);
                        self.add_project(ctx, path);
                    }
                    if ui.button("Annuler").clicked() { self.adding = false; }
                });
            });
        }
        if self.native_menu.is_none() && !self.adding && !self.settings_open
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers { shift: true, ..egui::Modifiers::COMMAND }, egui::Key::P)) {
            self.toggle_palette(ctx);
        }
        if self.palette.open {
            let entries = self.palette_entries();
            let target=if self.compose.active {
                self.palette.show_named(ctx,&entries,"Docker Compose",&self.compose.subtitle())
            }else{self.palette.show(ctx,&entries)};
            if let Some(target) = target {
                match target {
                    palette::Target::Compose(action) => self.compose_action(ctx,action),
                    palette::Target::Action(action) => self.execute_action(ctx, action),
                    palette::Target::Directory(directory) => {
                        if let Some(index) = self.projects.iter().position(|p| p.directory == directory) {
                            self.selected = index;
                        } else { self.add_project(ctx, directory); }
                    }
                    palette::Target::Pane(id) | palette::Target::Project(id) => {
                        for (index, project) in self.projects.iter_mut().enumerate() {
                            if let Some(pane) = project.panes.iter().position(|pane| pane.backend.id == id) {
                                self.selected = index;
                                project.focused = pane;
                                break;
                            }
                        }
                    }
                }
                ctx.request_repaint();
            }
        }
        if self.compose.configure {
            egui::Window::new("Fichiers Compose du projet").collapsible(false).resizable(false).show(ctx,|ui|{
                ui.label("Un chemin par ligne, relatif au projet ou absolu. Vide : découverte Compose automatique.");
                ui.add(egui::TextEdit::multiline(&mut self.compose.file_input).desired_width(500.0));
                if ui.button("Utiliser et actualiser").clicked(){
                    self.compose.files=self.compose.file_input.lines().map(str::trim).filter(|s|!s.is_empty()).map(PathBuf::from).collect();
                    if let Some(root)=self.compose.root.clone(){if self.compose.files.is_empty(){self.compose.file_selections.remove(&root);}else{self.compose.file_selections.insert(root,self.compose.files.clone());}}
                    self.compose.configure=false;self.compose.refresh(ctx.clone());self.palette.reset_open();
                }
                if ui.button("Annuler").clicked(){self.compose.configure=false;self.palette.reset_open();}
            });
        }
        if !self.palette.open && !self.compose.configure { self.palette_directories.clear();self.compose.leave(); }
        if let Some(menu) = &self.native_menu {
            menu.update(!self.closed_projects.is_empty(), self.projects.len(), self.projects.get(self.selected).map_or(0, |project| project.panes.len()), self.adding || self.settings_open || self.palette.open || self.compose.configure, self.palette.open, self.sidebar_visible,
                self.projects.get(self.selected).is_some_and(|project| project.zoomed));
        }
        self.persist(ctx);
    }
}

fn main() -> eframe::Result {
    std::env::set_var("TERM", "xterm-256color");
    std::env::set_var("COLORTERM", "truecolor");
    eframe::run_native("Atelier — Terminal", eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_fullsize_content_view(true).with_title_shown(false).with_titlebar_shown(false).with_titlebar_buttons_shown(true).with_inner_size([1100.0, 720.0]).with_min_inner_size([680.0, 400.0]),
        ..Default::default()
    }, Box::new(|cc| Ok(Box::new(Atelier::new(cc)))))
}
