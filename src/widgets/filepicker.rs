use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::t;
use crate::theme::Theme;
use crate::widgets::list::ListSelection;
use crate::widgets::popup::{Popup, PopupEvent};
use crate::widgets::text_edit::{EditEvent, TextEdit};

const MIN_WIDTH: u16 = 48;
const MIN_HEIGHT: u16 = 10;
const WIDTH_PERCENT: u32 = 80;
const HEIGHT_PERCENT: u32 = 75;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerMode {
    File,
    Directory,
}

#[derive(Debug, Clone)]
pub struct FilePickerOptions {
    pub cwd: PathBuf,
    pub preferred_extensions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    Parent,
    Directory,
    File,
}

#[derive(Debug, Clone)]
struct Entry {
    path: PathBuf,
    name: String,
    kind: EntryKind,
    preferred: bool,
}

/// A modal file or directory browser that owns its navigation state.
#[derive(Debug)]
pub struct FilePicker {
    title: String,
    mode: FilePickerMode,
    directory: PathBuf,
    entries: Vec<Entry>,
    visible: Vec<usize>,
    selected: ListSelection,
    filter: Option<TextEdit>,
    preferred_extensions: Vec<String>,
    error: Option<String>,
}

impl FilePicker {
    pub fn open(
        options: FilePickerOptions,
        mode: FilePickerMode,
        title: impl Into<String>,
    ) -> Self {
        let directory = initial_directory(&options.cwd);
        let mut picker = Self {
            title: title.into(),
            mode,
            directory: directory.clone(),
            entries: Vec::new(),
            visible: Vec::new(),
            selected: ListSelection::wrap(0),
            filter: None,
            preferred_extensions: options
                .preferred_extensions
                .iter()
                .map(|extension| normalize_extension(extension))
                .filter(|extension| !extension.is_empty())
                .collect(),
            error: None,
        };
        picker.load(directory);
        picker
    }

    fn load(&mut self, directory: PathBuf) {
        match read_entries(&directory, &self.preferred_extensions) {
            Ok(entries) => {
                self.directory = directory;
                self.entries = entries;
                self.filter = None;
                self.rebuild_visible();
                self.error = None;
            }
            Err(error) => self.error = Some(format!("{error:#}")),
        }
    }

    fn rebuild_visible(&mut self) {
        let query = self
            .filter
            .as_ref()
            .map(|filter| filter.input().to_lowercase())
            .unwrap_or_default();
        self.visible = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| query.is_empty() || entry.name.to_lowercase().contains(&query))
            .map(|(index, _)| index)
            .collect();
        self.selected.select(Some(0));
        self.selected.set_len(self.visible.len());
    }

    fn clear_filter(&mut self) {
        self.filter = None;
        self.rebuild_visible();
    }

    fn selected_entry(&self) -> Option<&Entry> {
        self.visible
            .get(self.selected.index())
            .and_then(|index| self.entries.get(*index))
    }

    fn activate(&mut self) -> PopupEvent<PathBuf> {
        let Some(entry) = self.selected_entry().cloned() else {
            return PopupEvent::Handled;
        };

        match entry.kind {
            EntryKind::Parent | EntryKind::Directory => {
                self.load(entry.path);
                PopupEvent::Handled
            }
            EntryKind::File if self.mode == FilePickerMode::File => {
                PopupEvent::Completed(entry.path)
            }
            EntryKind::File => PopupEvent::Handled,
        }
    }

    fn confirm(&mut self) -> PopupEvent<PathBuf> {
        match self.mode {
            FilePickerMode::Directory => PopupEvent::Completed(self.directory.clone()),
            FilePickerMode::File => self
                .selected_entry()
                .filter(|entry| entry.kind == EntryKind::File)
                .map(|entry| PopupEvent::Completed(entry.path.clone()))
                .unwrap_or(PopupEvent::Handled),
        }
    }

    fn go_parent(&mut self) {
        if let Some(parent) = self.directory.parent() {
            self.load(parent.to_path_buf());
        }
    }
}

impl Popup for FilePicker {
    type Value = PathBuf;

    fn handle_key(&mut self, key: KeyEvent) -> PopupEvent<PathBuf> {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return PopupEvent::Ignored;
        }

        if let Some(filter) = self.filter.as_mut() {
            if self.selected.handle_key(key.code) {
                return PopupEvent::Handled;
            }
            return match filter.handle_key(key) {
                EditEvent::Changed => {
                    self.rebuild_visible();
                    PopupEvent::Handled
                }
                EditEvent::Submit => self.activate(),
                EditEvent::Cancel => {
                    self.clear_filter();
                    PopupEvent::Handled
                }
                EditEvent::Unhandled => PopupEvent::Handled,
            };
        }

        match key.code {
            KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End => {
                self.selected.handle_key(key.code);
                PopupEvent::Handled
            }
            KeyCode::Left => {
                self.go_parent();
                PopupEvent::Handled
            }
            KeyCode::Right | KeyCode::Enter => self.activate(),
            KeyCode::Char(' ') => self.confirm(),
            KeyCode::Char('/') => {
                self.filter = Some(TextEdit::new(String::new()));
                PopupEvent::Handled
            }
            KeyCode::Esc => PopupEvent::Cancelled,
            _ => PopupEvent::Handled,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, _anchor: Rect, theme: Theme) {
        let popup = popup_area(area);
        frame.render_widget(Clear, popup);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(theme.border_focused))
            .title(self.title.clone());
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        if inner.is_empty() {
            return;
        }

        let [path_area, filter_area, content_area, footer_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(u16::from(self.filter.is_some())),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(inner);

        let path = Line::from(Span::styled(
            self.directory.display().to_string(),
            Style::new().fg(theme.text_muted),
        ));
        frame.render_widget(Paragraph::new(path), path_area);

        if let Some(filter) = &self.filter {
            let line = Line::from(vec![
                Span::styled(
                    format!("{}: ", t!("common.filter")),
                    Style::new().fg(theme.accent),
                ),
                Span::raw(filter.cursor_display()),
            ]);
            frame.render_widget(Paragraph::new(line), filter_area);
        }

        let items = self
            .visible
            .iter()
            .filter_map(|index| self.entries.get(*index))
            .map(|entry| ListItem::new(entry_line(entry, theme)))
            .collect::<Vec<ListItem>>();
        let list = List::new(items)
            .highlight_style(theme.selection_style())
            .highlight_symbol("▸ ");

        let rows = content_area.height as usize;
        let selected = self
            .selected
            .index()
            .min(self.visible.len().saturating_sub(1));
        let offset = selected.saturating_add(1).saturating_sub(rows);
        let mut list_state = ListState::default()
            .with_selected((!self.visible.is_empty()).then_some(selected))
            .with_offset(offset);
        if self.visible.is_empty() && self.filter.is_some() {
            frame.render_widget(
                Paragraph::new(t!("common.no_matches"))
                    .centered()
                    .style(Style::new().fg(theme.text_dim)),
                content_area,
            );
        } else {
            frame.render_stateful_widget(list, content_area, &mut list_state);
        }

        let footer = match &self.error {
            Some(error) => Line::styled(error.clone(), Style::new().fg(theme.danger)),
            None => Line::raw(""),
        };
        frame.render_widget(Paragraph::new(footer), footer_area);
    }

    fn hints(&self) -> Vec<KeyHint> {
        if self.filter.is_some() {
            vec![
                KeyHint::new("↑/↓", t!("common.select")),
                KeyHint::new("ENTER", t!("common.open")),
                KeyHint::new("ESC", t!("common.cancel")),
            ]
        } else {
            vec![
                KeyHint::new("↑/↓", t!("common.select")),
                KeyHint::new("ENTER", t!("common.open")),
                KeyHint::new("SPACE", t!("common.select")),
                KeyHint::new("/", t!("common.filter")),
                KeyHint::new("ESC", t!("common.cancel")),
            ]
        }
    }
}

fn initial_directory(cwd: &Path) -> PathBuf {
    let candidate = if cwd.is_dir() {
        cwd.to_path_buf()
    } else {
        cwd.parent()
            .filter(|parent| parent.is_dir())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    };
    std::path::absolute(&candidate).unwrap_or(candidate)
}

fn read_entries(directory: &Path, preferred_extensions: &[String]) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    if let Some(parent) = directory.parent() {
        entries.push(Entry {
            path: parent.to_path_buf(),
            name: "..".to_owned(),
            kind: EntryKind::Parent,
            preferred: false,
        });
    }

    let read = fs::read_dir(directory)
        .with_context(|| format!("failed to read directory: {}", directory.display()))?;
    for entry in read {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", directory.display()))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .with_context(|| format!("failed to read metadata: {}", path.display()))?;
        let kind = if metadata.is_dir() {
            EntryKind::Directory
        } else if metadata.is_file() {
            EntryKind::File
        } else {
            continue;
        };
        let preferred = kind == EntryKind::File && has_extension(&path, preferred_extensions);
        entries.push(Entry {
            path,
            name: entry.file_name().to_string_lossy().into_owned(),
            kind,
            preferred,
        });
    }

    entries.sort_by_cached_key(|entry| (entry_rank(entry), entry.name.to_lowercase()));
    Ok(entries)
}

const fn entry_rank(entry: &Entry) -> u8 {
    match entry.kind {
        EntryKind::Parent => 0,
        EntryKind::Directory => 1,
        EntryKind::File if entry.preferred => 2,
        EntryKind::File => 3,
    }
}

fn entry_line(entry: &Entry, theme: Theme) -> Line<'static> {
    match entry.kind {
        EntryKind::Parent => Line::styled("..", Style::new().fg(theme.text_dim)),
        EntryKind::Directory => {
            Line::styled(format!("{}/", entry.name), Style::new().fg(theme.info))
        }
        EntryKind::File if entry.preferred => Line::styled(
            entry.name.clone(),
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        ),
        EntryKind::File => Line::raw(entry.name.clone()),
    }
}

fn normalize_extension(extension: &str) -> String {
    extension.trim_start_matches('.').to_ascii_lowercase()
}

fn has_extension(path: &Path, extensions: &[String]) -> bool {
    path.extension()
        .and_then(|found| found.to_str())
        .is_some_and(|found| {
            extensions
                .iter()
                .any(|extension| found.eq_ignore_ascii_case(extension))
        })
}

fn popup_area(area: Rect) -> Rect {
    let width = (u32::from(area.width) * WIDTH_PERCENT / 100) as u16;
    let height = (u32::from(area.height) * HEIGHT_PERCENT / 100) as u16;
    let width = width.max(MIN_WIDTH).min(area.width);
    let height = height.max(MIN_HEIGHT).min(area.height);

    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}
