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
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::t;
use crate::theme::Theme;
use crate::widgets::popup::{Popup, PopupEvent};

const MIN_WIDTH: u16 = 48;
const MIN_HEIGHT: u16 = 10;
const WIDTH_PERCENT: u32 = 80;
const HEIGHT_PERCENT: u32 = 75;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerMode {
    File,
    Directory,
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
}

/// A modal file or directory browser that owns its navigation state.
#[derive(Debug)]
pub struct FilePicker {
    title: String,
    mode: FilePickerMode,
    directory: PathBuf,
    entries: Vec<Entry>,
    selected: usize,
    error: Option<String>,
}

impl FilePicker {
    pub fn open(start: &Path, mode: FilePickerMode, title: impl Into<String>) -> Self {
        let directory = initial_directory(start);
        let mut picker = Self {
            title: title.into(),
            mode,
            directory: directory.clone(),
            entries: Vec::new(),
            selected: 0,
            error: None,
        };
        picker.load(directory);
        picker
    }

    fn load(&mut self, directory: PathBuf) {
        match read_entries(&directory) {
            Ok(entries) => {
                self.directory = directory;
                self.entries = entries;
                self.selected = 0;
                self.error = None;
            }
            Err(error) => self.error = Some(format!("{error:#}")),
        }
    }

    fn move_selection(&mut self, forward: bool) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = if forward {
            (self.selected + 1) % self.entries.len()
        } else if self.selected == 0 {
            self.entries.len() - 1
        } else {
            self.selected - 1
        };
    }

    fn activate(&mut self) -> PopupEvent<PathBuf> {
        let Some(entry) = self.entries.get(self.selected).cloned() else {
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
                .entries
                .get(self.selected)
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

        match key.code {
            KeyCode::Up => {
                self.move_selection(false);
                PopupEvent::Handled
            }
            KeyCode::Down => {
                self.move_selection(true);
                PopupEvent::Handled
            }
            KeyCode::Home => {
                self.selected = 0;
                PopupEvent::Handled
            }
            KeyCode::End => {
                self.selected = self.entries.len().saturating_sub(1);
                PopupEvent::Handled
            }
            KeyCode::Left => {
                self.go_parent();
                PopupEvent::Handled
            }
            KeyCode::Right | KeyCode::Enter => self.activate(),
            KeyCode::Char(' ') => self.confirm(),
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

        let [path_area, list_area, footer_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(inner);

        let path = Line::from(Span::styled(
            self.directory.display().to_string(),
            Style::new().fg(theme.text_muted),
        ));
        frame.render_widget(Paragraph::new(path), path_area);

        let items = self
            .entries
            .iter()
            .map(|entry| ListItem::new(entry_line(entry, theme)))
            .collect::<Vec<ListItem>>();
        let list = List::new(items)
            .highlight_style(theme.selection_style())
            .highlight_symbol("▸ ");

        let visible = list_area.height as usize;
        let selected = self.selected.min(self.entries.len().saturating_sub(1));
        let offset = selected.saturating_add(1).saturating_sub(visible);
        let mut list_state = ListState::default()
            .with_selected((!self.entries.is_empty()).then_some(selected))
            .with_offset(offset);
        frame.render_stateful_widget(list, list_area, &mut list_state);

        let footer = match &self.error {
            Some(error) => Line::styled(error.clone(), Style::new().fg(theme.danger)),
            None => Line::raw(""),
        };
        frame.render_widget(Paragraph::new(footer), footer_area);
    }

    fn hints(&self) -> Vec<KeyHint> {
        vec![
            KeyHint::new("↑/↓", t!("common.select")),
            KeyHint::new("ENTER", t!("common.open")),
            KeyHint::new("SPACE", t!("common.select")),
            KeyHint::new("ESC", t!("common.cancel")),
        ]
    }
}

fn initial_directory(start: &Path) -> PathBuf {
    let candidate = if start.is_dir() {
        start.to_path_buf()
    } else {
        start
            .parent()
            .filter(|parent| parent.is_dir())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    };
    std::path::absolute(&candidate).unwrap_or(candidate)
}

fn read_entries(directory: &Path) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    if let Some(parent) = directory.parent() {
        entries.push(Entry {
            path: parent.to_path_buf(),
            name: "..".to_owned(),
            kind: EntryKind::Parent,
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
        entries.push(Entry {
            path,
            name: entry.file_name().to_string_lossy().into_owned(),
            kind,
        });
    }

    entries.sort_by_cached_key(|entry| (kind_rank(entry.kind), entry.name.to_lowercase()));
    Ok(entries)
}

const fn kind_rank(kind: EntryKind) -> u8 {
    match kind {
        EntryKind::Parent => 0,
        EntryKind::Directory => 1,
        EntryKind::File => 2,
    }
}

fn entry_line(entry: &Entry, theme: Theme) -> Line<'static> {
    match entry.kind {
        EntryKind::Parent => Line::styled("..", Style::new().fg(theme.text_dim)),
        EntryKind::Directory => {
            Line::styled(format!("{}/", entry.name), Style::new().fg(theme.info))
        }
        EntryKind::File => Line::raw(entry.name.clone()),
    }
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
