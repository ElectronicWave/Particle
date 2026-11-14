use std::path::PathBuf;

use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};

use crate::app::{
    config::{LauncherConfig, validate_custom_runtime},
    services::Services,
};
use crate::pages::content::{Content, ContentEvent};
use crate::theme::Theme;
use crate::widgets::tabs::TabBar;

const SETTINGS_HINTS: &[KeyHint] = &[
    KeyHint::from_static("TAB", "Switch tab"),
    KeyHint::from_static("↑/↓", "Field"),
    KeyHint::from_static("ENTER", "Edit"),
    KeyHint::from_static("←/→", "Change"),
    KeyHint::from_static("ESC", "Return"),
];

const EDIT_HINTS: &[KeyHint] = &[
    KeyHint::from_static("ENTER", "Save"),
    KeyHint::from_static("ESC", "Cancel"),
];

const JAVA_MAJORS: [Option<usize>; 6] = [None, Some(8), Some(11), Some(17), Some(21), Some(25)];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum SettingsTab {
    #[default]
    General,
    Runtime,
    Downloads,
}

impl SettingsTab {
    const ALL: [SettingsTab; 3] = [Self::General, Self::Runtime, Self::Downloads];

    const fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Runtime => "Runtime",
            Self::Downloads => "Downloads",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::General => Self::Runtime,
            Self::Runtime => Self::Downloads,
            Self::Downloads => Self::General,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::General => Self::Downloads,
            Self::Runtime => Self::General,
            Self::Downloads => Self::Runtime,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditTarget {
    StorageRoot,
    AddRuntime,
    Runtime(usize),
}

#[derive(Debug)]
struct Edit {
    target: EditTarget,
    input: String,
}

#[derive(Debug, Default)]
pub struct SettingsContent {
    tab: SettingsTab,
    selected: usize,
    runtime_rows: usize,
    edit: Option<Edit>,
    error: Option<String>,
}

impl Content for SettingsContent {
    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect) {
        let theme = ctx.theme();
        let [tabs_area, list_area, status_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(2),
        ])
        .areas(area);

        let labels = SettingsTab::ALL.map(|tab| tab.label());
        let index = SettingsTab::ALL
            .iter()
            .position(|tab| *tab == self.tab)
            .unwrap_or(0);
        frame.render_widget(TabBar::new(theme, &labels, index), tabs_area);

        let config = ctx.config();
        self.runtime_rows = config.custom_runtime_paths.len();
        self.clamp_selection(&config);
        let items = self
            .rows(&config, theme)
            .into_iter()
            .map(ListItem::new)
            .collect::<Vec<ListItem>>();
        let list = List::new(items).highlight_style(
            Style::new()
                .fg(theme.selection_fg)
                .bg(theme.selection_bg)
                .add_modifier(Modifier::BOLD),
        );
        let mut state = ListState::default().with_selected(Some(self.selected));
        frame.render_stateful_widget(list, list_area, &mut state);

        frame.render_widget(
            Paragraph::new(self.status_line(ctx, &config, theme)),
            status_area,
        );
    }

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> ContentEvent {
        if self.edit.is_some() {
            return self.on_edit_key(ctx, key);
        }

        match key.code {
            KeyCode::Tab => {
                self.tab = self.tab.next();
                self.selected = 0;
                ContentEvent::Handled
            }
            KeyCode::BackTab => {
                self.tab = self.tab.previous();
                self.selected = 0;
                ContentEvent::Handled
            }
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                ContentEvent::Handled
            }
            KeyCode::Down => {
                let count = self.field_count(&ctx.config());
                self.selected = self.selected.saturating_add(1).min(count - 1);
                ContentEvent::Handled
            }
            KeyCode::Enter => {
                self.activate(ctx);
                ContentEvent::Handled
            }
            KeyCode::Left => {
                self.adjust(ctx, false);
                ContentEvent::Handled
            }
            KeyCode::Right => {
                self.adjust(ctx, true);
                ContentEvent::Handled
            }
            KeyCode::Delete => {
                self.remove_runtime(ctx);
                ContentEvent::Handled
            }
            _ => ContentEvent::Ignored,
        }
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        if self.edit.is_some() {
            return EDIT_HINTS.to_vec();
        }

        let mut hints = SETTINGS_HINTS.to_vec();
        if self.tab == SettingsTab::Runtime
            && self.selected >= 2
            && self.selected < 2 + self.runtime_rows
        {
            hints.insert(3, KeyHint::from_static("DEL", "Remove"));
        }
        hints
    }
}

impl SettingsContent {
    fn rows(&self, config: &LauncherConfig, theme: Theme) -> Vec<Line<'static>> {
        match self.tab {
            SettingsTab::General => self.general_rows(config, theme),
            SettingsTab::Runtime => self.runtime_rows(config, theme),
            SettingsTab::Downloads => self.download_rows(config, theme),
        }
    }

    fn general_rows(&self, config: &LauncherConfig, theme: Theme) -> Vec<Line<'static>> {
        let storage_root = self
            .edit_value(EditTarget::StorageRoot)
            .unwrap_or_else(|| config.storage_root.display().to_string());
        vec![
            self.row(0, "Storage root", storage_root, theme),
            self.row(
                1,
                "Config file",
                config.config_path().display().to_string(),
                theme,
            ),
        ]
    }

    fn runtime_rows(&self, config: &LauncherConfig, theme: Theme) -> Vec<Line<'static>> {
        let java = config
            .java_major_version
            .map(|major| format!("Java {major}"))
            .unwrap_or_else(|| "Automatic".to_owned());
        let mut rows = vec![
            self.row(0, "Java runtime", java, theme),
            self.row(
                1,
                "Validation",
                config.runtime_validation.label().to_owned(),
                theme,
            ),
        ];

        for (index, path) in config.custom_runtime_paths.iter().enumerate() {
            let value = self
                .edit_value(EditTarget::Runtime(index))
                .unwrap_or_else(|| path.display().to_string());
            rows.push(self.row(2 + index, &format!("Custom {}", index + 1), value, theme));
        }

        let add_index = 2 + config.custom_runtime_paths.len();
        let add_value = self
            .edit_value(EditTarget::AddRuntime)
            .unwrap_or_else(|| "Enter to import a Java home".to_owned());
        rows.push(self.row(add_index, "Add runtime", add_value, theme));
        rows
    }

    fn download_rows(&self, config: &LauncherConfig, theme: Theme) -> Vec<Line<'static>> {
        vec![self.row(
            0,
            "Parallel downloads",
            format!("{}", config.download_parallelism),
            theme,
        )]
    }

    fn row(&self, index: usize, label: &str, value: String, theme: Theme) -> Line<'static> {
        let style = if index == self.selected {
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::new()
        };
        Line::from(vec![
            Span::styled(format!("{label:<20}"), style),
            Span::raw(value),
        ])
    }

    fn edit_value(&self, target: EditTarget) -> Option<String> {
        self.edit
            .as_ref()
            .filter(|edit| edit.target == target)
            .map(|edit| format!("{}█", edit.input))
    }

    fn status_line(&self, ctx: &Services, config: &LauncherConfig, theme: Theme) -> Line<'static> {
        if self.edit.is_some() {
            return Line::styled(
                "Enter saves, Esc cancels. Custom runtimes must point to a Java home containing bin/java.",
                Style::new().fg(theme.text_dim),
            );
        }

        if let Some(error) = &self.error {
            return Line::styled(error.clone(), Style::new().fg(theme.danger));
        }

        if let Some(error) = ctx.config_error() {
            return Line::styled(
                format!("Could not save config: {error}"),
                Style::new().fg(theme.danger),
            );
        }

        Line::styled(
            format!(
                "Saved to {}. Instance overrides live in versions/<name>/.elemental/particle.toml.",
                config.config_path().display()
            ),
            Style::new().fg(theme.text_dim),
        )
    }

    fn activate(&mut self, ctx: &Services) {
        match self.tab {
            SettingsTab::General => {
                if self.selected == 0 {
                    let storage_root = ctx.config().storage_root.display().to_string();
                    self.start_edit(EditTarget::StorageRoot, storage_root);
                }
            }
            SettingsTab::Runtime => {
                let config = ctx.config();
                let count = config.custom_runtime_paths.len();
                match self.selected {
                    0 => {
                        let mut config = config;
                        config.java_major_version = cycle_java(config.java_major_version, true);
                        self.apply(ctx, config);
                    }
                    1 => {
                        let mut config = config;
                        config.runtime_validation = config.runtime_validation.toggled();
                        self.apply(ctx, config);
                    }
                    index if index >= 2 && index < 2 + count => {
                        let path = config.custom_runtime_paths[index - 2].display().to_string();
                        self.start_edit(EditTarget::Runtime(index - 2), path);
                    }
                    _ => self.start_edit(EditTarget::AddRuntime, String::new()),
                }
            }
            SettingsTab::Downloads => self.adjust(ctx, true),
        }
    }

    fn adjust(&mut self, ctx: &Services, forward: bool) {
        let mut config = ctx.config();
        match self.tab {
            SettingsTab::Runtime if self.selected == 0 => {
                config.java_major_version = cycle_java(config.java_major_version, forward);
            }
            SettingsTab::Downloads if self.selected == 0 => {
                config.download_parallelism = if forward {
                    config.download_parallelism.saturating_add(1).min(64)
                } else {
                    config.download_parallelism.saturating_sub(1).max(1)
                };
            }
            _ => return,
        }
        self.apply(ctx, config);
    }

    fn remove_runtime(&mut self, ctx: &Services) {
        if self.tab != SettingsTab::Runtime
            || self.selected < 2
            || self.selected >= 2 + ctx.config().custom_runtime_paths.len()
        {
            return;
        }

        let mut config = ctx.config();
        config.custom_runtime_paths.remove(self.selected - 2);
        self.selected = self.selected.min(self.field_count(&config) - 1);
        self.apply(ctx, config);
    }

    fn start_edit(&mut self, target: EditTarget, input: String) {
        self.error = None;
        self.edit = Some(Edit { target, input });
    }

    fn on_edit_key(&mut self, ctx: &Services, key: KeyEvent) -> ContentEvent {
        if self.edit.is_none() {
            return ContentEvent::Ignored;
        }

        match key.code {
            KeyCode::Esc => {
                self.edit = None;
                self.error = None;
            }
            KeyCode::Backspace => {
                if let Some(edit) = &mut self.edit {
                    edit.input.pop();
                }
            }
            KeyCode::Enter => {
                let edit = self.edit.take().expect("checked above");
                self.commit_edit(ctx, edit);
            }
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(edit) = &mut self.edit {
                    edit.input.push(character);
                }
            }
            _ => {}
        }

        ContentEvent::Handled
    }

    fn commit_edit(&mut self, ctx: &Services, edit: Edit) {
        let value = edit.input.trim();
        if value.is_empty() {
            self.error = Some("Path cannot be empty.".to_owned());
            return;
        }

        let mut config = ctx.config();
        match edit.target {
            EditTarget::StorageRoot => {
                config.storage_root = PathBuf::from(value);
            }
            EditTarget::AddRuntime => match validate_custom_runtime(&PathBuf::from(value)) {
                Ok(path) => {
                    if !config.custom_runtime_paths.contains(&path) {
                        config.custom_runtime_paths.push(path);
                        self.selected = 2 + config.custom_runtime_paths.len() - 1;
                    }
                }
                Err(error) => {
                    self.error = Some(format!("{error:#}"));
                    return;
                }
            },
            EditTarget::Runtime(index) => match validate_custom_runtime(&PathBuf::from(value)) {
                Ok(path) => {
                    if let Some(slot) = config.custom_runtime_paths.get_mut(index) {
                        *slot = path;
                    }
                }
                Err(error) => {
                    self.error = Some(format!("{error:#}"));
                    return;
                }
            },
        }

        self.apply(ctx, config);
    }

    fn apply(&mut self, ctx: &Services, config: LauncherConfig) {
        match ctx.update_config(config) {
            Ok(()) => self.error = None,
            Err(error) => self.error = Some(error),
        }
    }

    fn field_count(&self, config: &LauncherConfig) -> usize {
        match self.tab {
            SettingsTab::General => 2,
            SettingsTab::Runtime => 3 + config.custom_runtime_paths.len(),
            SettingsTab::Downloads => 1,
        }
    }

    fn clamp_selection(&mut self, config: &LauncherConfig) {
        self.selected = self.selected.min(self.field_count(config) - 1);
    }
}

fn cycle_java(current: Option<usize>, forward: bool) -> Option<usize> {
    let index = JAVA_MAJORS
        .iter()
        .position(|major| *major == current)
        .unwrap_or(0);
    let next = if forward {
        (index + 1) % JAVA_MAJORS.len()
    } else {
        (index + JAVA_MAJORS.len() - 1) % JAVA_MAJORS.len()
    };
    JAVA_MAJORS[next]
}
