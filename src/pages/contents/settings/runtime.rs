use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use elemental::core::runtime::distribution::Distribution;
use ratail::{KeyHint, SnapshotStatus};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    style::Style,
    text::{Line, Span},
};

use crate::app::{
    config::{LauncherConfig, validate_custom_runtime},
    services::Services,
};
use crate::t;
use crate::theme::Theme;
use crate::widgets::{
    cycle::Cycle,
    dropdown::Dropdown,
    filepicker::{FilePicker, FilePickerMode},
    popup::{Popup, PopupEvent},
    spinner,
};

use super::pane::{self, PaneAreas, PaneEvent, RowHint, SettingsPane};

const FIELD_COUNT: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeChoice {
    Auto,
    Path(PathBuf),
    Add,
}

#[derive(Debug, Default)]
pub(super) struct RuntimePane {
    selected: usize,
    runtime_dropdown: Option<Dropdown<RuntimeChoice>>,
    picker: Option<FilePicker>,
    custom_runtime_paths: Vec<PathBuf>,
    error: Option<String>,
}

impl SettingsPane for RuntimePane {
    fn title(&self) -> Cow<'static, str> {
        t!("settings.tab_runtime")
    }

    fn init(&mut self, ctx: &Services) {
        if ctx.runtimes().snapshot().view().status == SnapshotStatus::Idle {
            ctx.load_runtimes();
        }
    }

    fn render(&mut self, ctx: &Services, frame: &mut Frame, areas: PaneAreas, theme: Theme) {
        let config = ctx.config();
        self.custom_runtime_paths = config.custom_runtime_paths.clone();

        let snapshot = ctx.runtimes().snapshot();
        let runtimes = snapshot
            .view()
            .data
            .map(|runtimes| runtimes.as_slice())
            .unwrap_or(&[]);
        let java = choice_label(&current_choice(&config), runtimes);
        let rows = vec![
            pane::row(0, self.selected, &t!("settings.java_runtime"), java, theme),
            pane::row(
                1,
                self.selected,
                &t!("settings.validation"),
                config.runtime_validation.label().into_owned(),
                theme,
            ),
        ];

        pane::render_rows(frame, areas.body, rows, self.selected, theme);
        let anchor = pane::row_anchor(areas.body);
        if let Some(popup) = &self.runtime_dropdown {
            popup.render(frame, areas.full, anchor, theme);
        }
        if let Some(popup) = &self.picker {
            popup.render(frame, areas.full, anchor, theme);
        }
    }

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> PaneEvent {
        if let Some(popup) = &mut self.picker {
            match popup.handle_key(key) {
                PopupEvent::Ignored => {}
                PopupEvent::Handled => return PaneEvent::Handled,
                PopupEvent::Cancelled => {
                    self.picker = None;
                    return PaneEvent::Handled;
                }
                PopupEvent::Completed(path) => {
                    self.picker = None;
                    if self.add_runtime(ctx, path) {
                        self.runtime_dropdown = None;
                    }
                    return PaneEvent::Handled;
                }
            }
        }

        if self.runtime_dropdown.is_some() {
            if key.code == KeyCode::Delete
                && !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
            {
                if let Some(path) = self.selected_custom_runtime(ctx) {
                    self.runtime_dropdown = None;
                    self.remove_custom_runtime(ctx, &path);
                    return PaneEvent::Handled;
                }
            }

            let popup = self
                .runtime_dropdown
                .as_mut()
                .expect("runtime dropdown checked above");
            match popup.handle_key(key) {
                PopupEvent::Ignored => {}
                PopupEvent::Handled => return PaneEvent::Handled,
                PopupEvent::Cancelled => {
                    self.runtime_dropdown = None;
                    return PaneEvent::Handled;
                }
                PopupEvent::Completed(choice) => {
                    self.select_runtime(ctx, choice);
                    return PaneEvent::Handled;
                }
            }
        }

        match key.code {
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                PaneEvent::Handled
            }
            KeyCode::Down => {
                self.selected = self.selected.saturating_add(1).min(FIELD_COUNT - 1);
                PaneEvent::Handled
            }
            KeyCode::Enter => {
                self.activate(ctx);
                PaneEvent::Handled
            }
            KeyCode::Left => {
                self.adjust(ctx, false);
                PaneEvent::Handled
            }
            KeyCode::Right => {
                self.adjust(ctx, true);
                PaneEvent::Handled
            }
            KeyCode::Char('r' | 'R')
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                ctx.load_runtimes();
                PaneEvent::Handled
            }
            _ => PaneEvent::Ignored,
        }
    }

    fn hints(&self) -> Vec<KeyHint> {
        if let Some(popup) = &self.picker {
            return popup.hints();
        }
        if let Some(popup) = &self.runtime_dropdown {
            let mut hints = popup.hints();
            if self.selected_custom_runtime_hint() {
                hints.push(KeyHint::new("DEL", t!("settings.remove_custom_runtime")));
            }
            return hints;
        }

        let row = if self.selected == 0 {
            RowHint::Select
        } else {
            RowHint::Toggle
        };
        let mut hints = pane::row_hints(row);
        hints.push(KeyHint::new("R", t!("common.refresh")));
        hints
    }

    fn is_modal(&self) -> bool {
        self.runtime_dropdown.is_some() || self.picker.is_some()
    }

    fn status(&self, ctx: &Services, theme: Theme) -> Option<Line<'static>> {
        if let Some(error) = &self.error {
            return Some(Line::styled(error.clone(), Style::new().fg(theme.danger)));
        }

        let snapshot = ctx.runtimes().snapshot();
        let view = snapshot.view();
        if view.status == SnapshotStatus::Loading {
            ctx.request_frame(spinner::INTERVAL);
            return Some(Line::from(vec![
                Span::raw(spinner::frame()),
                Span::styled(
                    format!(" {}", t!("settings.runtime_loading")),
                    Style::new().fg(theme.text_dim),
                ),
            ]));
        }
        if let Some(error) = view.error {
            return Some(Line::styled(
                t!(
                    "settings.runtime_detect_failed",
                    error = format!("{error:#}")
                ),
                Style::new().fg(theme.danger),
            ));
        }
        match view.data {
            Some(runtimes) => match current_choice(&ctx.config()) {
                RuntimeChoice::Path(path)
                    if runtimes.iter().any(|runtime| runtime.path() == path) =>
                {
                    Some(Line::styled(
                        t!(
                            "settings.runtime_selected_path",
                            path = path.display().to_string()
                        ),
                        Style::new().fg(theme.text_dim),
                    ))
                }
                RuntimeChoice::Path(path) => Some(Line::styled(
                    t!(
                        "settings.runtime_pinned_missing",
                        path = path.display().to_string()
                    ),
                    Style::new().fg(theme.danger),
                )),
                _ if runtimes.is_empty() => Some(Line::styled(
                    t!("settings.runtime_empty"),
                    Style::new().fg(theme.text_dim),
                )),
                _ => None,
            },
            None => None,
        }
    }
}

impl RuntimePane {
    fn activate(&mut self, ctx: &Services) {
        match self.selected {
            0 => self.open_runtime_dropdown(ctx),
            1 => {
                let mut config = ctx.config();
                config.runtime_validation = config.runtime_validation.toggled();
                self.apply(ctx, config);
            }
            _ => {}
        }
    }

    fn adjust(&mut self, ctx: &Services, forward: bool) {
        if self.selected != 0 {
            return;
        }

        let snapshot = ctx.runtimes().snapshot();
        let runtimes = snapshot
            .view()
            .data
            .map(|runtimes| runtimes.as_slice())
            .unwrap_or(&[]);
        let config = ctx.config();
        let current = current_choice(&config);
        let choices = runtime_cycle_choices(&config, &current, runtimes);
        let Some(choice) = Cycle::new(&choices).step(&current, forward).cloned() else {
            return;
        };
        self.select_runtime(ctx, choice);
    }

    fn open_runtime_dropdown(&mut self, ctx: &Services) {
        let snapshot = ctx.runtimes().snapshot();
        let runtimes = snapshot
            .view()
            .data
            .map(|runtimes| runtimes.as_slice())
            .unwrap_or(&[]);
        let config = ctx.config();
        let current = current_choice(&config);
        let mut choices = runtime_cycle_choices(&config, &current, runtimes);
        choices.push(RuntimeChoice::Add);
        let selected = choices
            .iter()
            .position(|choice| *choice == current)
            .unwrap_or(0);
        let options = choices
            .into_iter()
            .map(|choice| (choice_label(&choice, runtimes), choice))
            .collect();

        self.runtime_dropdown = Some(Dropdown::open(
            t!("settings.runtime_picker"),
            options,
            selected,
        ));
    }

    fn select_runtime(&mut self, ctx: &Services, choice: RuntimeChoice) {
        match choice {
            RuntimeChoice::Auto => {
                let mut config = ctx.config();
                config.java_runtime_path = None;
                self.runtime_dropdown = None;
                self.apply(ctx, config);
            }
            RuntimeChoice::Path(path) => {
                let mut config = ctx.config();
                config.java_runtime_path = Some(path);
                self.runtime_dropdown = None;
                self.apply(ctx, config);
            }
            RuntimeChoice::Add => self.open_picker(&ctx.config()),
        }
    }

    fn open_picker(&mut self, config: &LauncherConfig) {
        let start = std::env::current_dir().unwrap_or_else(|_| config.storage_root.clone());
        self.picker = Some(FilePicker::open(
            &start,
            FilePickerMode::Directory,
            t!("settings.pick_directory"),
        ));
    }

    fn add_runtime(&mut self, ctx: &Services, path: PathBuf) -> bool {
        let path = match validate_custom_runtime(&path) {
            Ok(path) => path,
            Err(error) => {
                self.error = Some(format!("{error:#}"));
                return false;
            }
        };

        let mut config = ctx.config();
        if !config
            .custom_runtime_paths
            .iter()
            .any(|existing| *existing == path)
        {
            config.custom_runtime_paths.push(path.clone());
        }
        config.java_runtime_path = Some(path);
        self.apply(ctx, config);
        self.error.is_none()
    }

    fn remove_custom_runtime(&mut self, ctx: &Services, path: &Path) {
        let mut config = ctx.config();
        let Some(index) = config
            .custom_runtime_paths
            .iter()
            .position(|custom| custom == path)
        else {
            return;
        };

        config.custom_runtime_paths.remove(index);
        if config.java_runtime_path.as_deref() == Some(path) {
            config.java_runtime_path = None;
        }
        self.apply(ctx, config);
    }

    fn selected_custom_runtime(&self, ctx: &Services) -> Option<PathBuf> {
        let RuntimeChoice::Path(path) = self.runtime_dropdown.as_ref()?.selected_value()? else {
            return None;
        };
        ctx.config()
            .custom_runtime_paths
            .iter()
            .find(|custom| custom.as_path() == path.as_path())
            .cloned()
    }

    fn selected_custom_runtime_hint(&self) -> bool {
        match self
            .runtime_dropdown
            .as_ref()
            .and_then(|dropdown| dropdown.selected_value())
        {
            Some(RuntimeChoice::Path(path)) => self
                .custom_runtime_paths
                .iter()
                .any(|custom| custom.as_path() == path.as_path()),
            _ => false,
        }
    }

    fn apply(&mut self, ctx: &Services, config: LauncherConfig) {
        match ctx.update_config(config) {
            Ok(()) => self.error = None,
            Err(error) => self.error = Some(error),
        }
    }
}

fn current_choice(config: &LauncherConfig) -> RuntimeChoice {
    if let Some(path) = &config.java_runtime_path {
        RuntimeChoice::Path(path.clone())
    } else {
        RuntimeChoice::Auto
    }
}

fn runtime_cycle_choices(
    config: &LauncherConfig,
    current: &RuntimeChoice,
    runtimes: &[Distribution],
) -> Vec<RuntimeChoice> {
    let mut choices = vec![RuntimeChoice::Auto];
    for runtime in runtimes {
        let choice = RuntimeChoice::Path(runtime.path().to_path_buf());
        if !choices.contains(&choice) {
            choices.push(choice);
        }
    }
    for path in &config.custom_runtime_paths {
        let choice = RuntimeChoice::Path(path.clone());
        if !choices.contains(&choice) {
            choices.push(choice);
        }
    }
    if !choices.contains(current) {
        choices.push(current.clone());
    }
    choices
}

fn choice_label(choice: &RuntimeChoice, runtimes: &[Distribution]) -> String {
    match choice {
        RuntimeChoice::Auto => t!("settings.runtime_auto").into_owned(),
        RuntimeChoice::Path(path) => runtimes
            .iter()
            .find(|runtime| runtime.path() == path)
            .map(runtime_label)
            .unwrap_or_else(|| path.display().to_string()),
        RuntimeChoice::Add => t!("settings.add_runtime").into_owned(),
    }
}

fn runtime_label(runtime: &Distribution) -> String {
    let release = runtime.release();
    let version = runtime
        .java_major_version()
        .map(|major| t!("common.java_version", major = major).into_owned())
        .or_else(|| release.and_then(|release| release.jre_version.clone()))
        .unwrap_or_else(|| t!("settings.runtime_unknown").into_owned());

    let mut parts = vec![version];
    if let Some(implementor) = release.and_then(|release| release.implementor.clone()) {
        parts.push(implementor);
    }
    if let Some(architecture) = release.and_then(|release| release.architecture.clone()) {
        parts.push(architecture);
    }
    parts.push(runtime.provider().to_owned());
    parts.join(" · ")
}
