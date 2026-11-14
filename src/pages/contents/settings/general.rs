use std::{borrow::Cow, path::PathBuf};

use ratail::{Context as _, KeyHint};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    text::Line,
};

use crate::app::{config::LauncherConfig, services::Services};
use crate::i18n;
use crate::t;
use crate::theme::Theme;
use crate::widgets::{
    cycle::Cycle,
    dropdown::Dropdown,
    filepicker::{FilePicker, FilePickerMode},
    popup::{Popup, PopupEvent},
    text_edit::{EditEvent, TextEdit},
};

use super::pane::{self, PaneAreas, PaneEvent, RowHint, SettingsPane};

#[derive(Debug, Default)]
pub(super) struct GeneralPane {
    selected: usize,
    edit: Option<TextEdit>,
    language: Option<Dropdown<String>>,
    picker: Option<FilePicker>,
    error: Option<String>,
}

impl SettingsPane for GeneralPane {
    fn title(&self) -> Cow<'static, str> {
        t!("settings.tab_general")
    }

    fn render(&mut self, ctx: &Services, frame: &mut Frame, areas: PaneAreas, theme: Theme) {
        let config = ctx.config();
        let storage_root = self
            .edit
            .as_ref()
            .map(TextEdit::cursor_display)
            .unwrap_or_else(|| config.storage_root.display().to_string());
        let rows = vec![
            pane::row(
                0,
                self.selected,
                &t!("settings.language"),
                current_language_name(),
                theme,
            ),
            pane::row(
                1,
                self.selected,
                &t!("settings.storage_root"),
                storage_root,
                theme,
            ),
        ];
        pane::render_rows(frame, areas.body, rows, self.selected, theme);
        let anchor = pane::row_anchor(areas.body);
        if let Some(popup) = &self.language {
            popup.render(frame, areas.full, anchor, theme);
        }
        if let Some(popup) = &self.picker {
            popup.render(frame, areas.full, anchor, theme);
        }
    }

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> PaneEvent {
        if let Some(popup) = &mut self.language {
            match popup.handle_key(key) {
                PopupEvent::Ignored => {}
                PopupEvent::Handled => return PaneEvent::Handled,
                PopupEvent::Cancelled => {
                    self.language = None;
                    return PaneEvent::Handled;
                }
                PopupEvent::Completed(code) => {
                    self.language = None;
                    self.select_language(ctx, code);
                    return PaneEvent::Handled;
                }
            }
        }

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
                    self.set_storage_root(ctx, path);
                    return PaneEvent::Handled;
                }
            }
        }

        if self.edit.is_some() {
            self.on_edit_key(ctx, key);
            return PaneEvent::Handled;
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
            KeyCode::Char('e' | 'E')
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if self.selected == 1 {
                    self.start_path_edit(&ctx.config());
                    PaneEvent::Handled
                } else {
                    PaneEvent::Ignored
                }
            }
            KeyCode::Left => {
                self.adjust(ctx, false);
                PaneEvent::Handled
            }
            KeyCode::Right => {
                self.adjust(ctx, true);
                PaneEvent::Handled
            }
            _ => PaneEvent::Ignored,
        }
    }

    fn hints(&self) -> Vec<KeyHint> {
        if let Some(popup) = &self.language {
            return popup.hints();
        }
        if let Some(popup) = &self.picker {
            return popup.hints();
        }
        if self.edit.is_some() {
            return pane::edit_hints();
        }
        pane::row_hints(self.row_hint())
    }

    fn is_modal(&self) -> bool {
        self.language.is_some() || self.picker.is_some() || self.edit.is_some()
    }

    fn status(&self, _ctx: &Services, theme: Theme) -> Option<Line<'static>> {
        if self.edit.is_some() {
            return Some(pane::edit_status(theme));
        }
        pane::error_status(self.error.as_ref(), theme)
    }
}

impl GeneralPane {
    fn activate(&mut self, ctx: &Services) {
        match self.selected {
            0 => self.open_language(),
            1 => self.open_picker(ctx.config().storage_root),
            _ => {}
        }
    }

    fn adjust(&mut self, ctx: &Services, forward: bool) {
        if self.selected != 0 {
            return;
        }

        let locales = i18n::available_locales();
        let codes = locales
            .iter()
            .map(|locale| locale.code.clone())
            .collect::<Vec<String>>();
        let current = i18n::current_locale();
        if let Some(code) = Cycle::new(&codes).step(&current, forward) {
            self.select_language(ctx, code.clone());
        }
    }

    fn open_language(&mut self) {
        let locales = i18n::available_locales();
        let current = i18n::current_locale();
        let selected = locales
            .iter()
            .position(|locale| locale.code == current)
            .unwrap_or(0);
        let options = locales
            .into_iter()
            .map(|locale| (locale.name, locale.code))
            .collect::<Vec<(String, String)>>();
        self.language = Some(Dropdown::open(
            t!("settings.language_picker"),
            options,
            selected,
        ));
    }

    fn open_picker(&mut self, start: PathBuf) {
        self.picker = Some(FilePicker::open(
            &start,
            FilePickerMode::Directory,
            t!("settings.pick_directory"),
        ));
    }

    fn start_path_edit(&mut self, config: &LauncherConfig) {
        self.error = None;
        self.edit = Some(TextEdit::new(config.storage_root.display().to_string()));
    }

    fn on_edit_key(&mut self, ctx: &Services, key: KeyEvent) {
        let event = match self.edit.as_mut() {
            Some(edit) => edit.handle_key(key),
            None => return,
        };

        match event {
            EditEvent::Submit => {
                let Some(edit) = self.edit.take() else {
                    return;
                };
                let value = edit.input().trim();
                if value.is_empty() {
                    self.error = Some(t!("settings.path_empty").into_owned());
                    return;
                }

                let mut config = ctx.config();
                config.storage_root = PathBuf::from(value);
                self.apply(ctx, config);
            }
            EditEvent::Cancel => {
                self.edit = None;
                self.error = None;
            }
            EditEvent::Unhandled | EditEvent::Changed => {}
        }
    }

    fn select_language(&mut self, ctx: &Services, code: String) {
        if let Err(error) = i18n::set_locale(&code) {
            self.error = Some(format!("{error:#}"));
            return;
        }

        let mut config = ctx.config();
        config.language = Some(i18n::current_locale());
        ctx.redraw().trigger();
        self.apply(ctx, config);
    }

    fn set_storage_root(&mut self, ctx: &Services, path: PathBuf) {
        let mut config = ctx.config();
        config.storage_root = path;
        self.apply(ctx, config);
    }

    fn apply(&mut self, ctx: &Services, config: LauncherConfig) {
        match ctx.update_config(config) {
            Ok(()) => self.error = None,
            Err(error) => self.error = Some(error),
        }
    }

    fn row_hint(&self) -> RowHint {
        match self.selected {
            0 => RowHint::Select,
            _ => RowHint::Browse,
        }
    }
}

const FIELD_COUNT: usize = 2;

fn current_language_name() -> String {
    let current = i18n::current_locale();
    i18n::available_locales()
        .iter()
        .find(|locale| locale.code == current)
        .map(|locale| locale.name.clone())
        .unwrap_or(current)
}
