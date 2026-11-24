use std::{borrow::Cow, path::PathBuf};

use ratail::{Context as _, InputEvent, KeyHint, Modal, ModalOutcome};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyModifiers},
    text::Line,
};

use crate::app::config::LauncherConfig;
use crate::i18n;
use crate::services::Services;
use crate::t;
use crate::theme::Theme;
use crate::widgets::{
    cycle::Cycle,
    dropdown::Dropdown,
    filepicker::{FilePicker, FilePickerMode, FilePickerOptions},
    list::{ListAction, ListSelection},
    text_edit::{EditEvent, TextEdit},
};

use super::pane::{self, PaneAreas, PaneEvent, RowHint, SettingsPane};

#[derive(Debug, Default)]
pub(super) struct GeneralPane {
    selected: ListSelection,
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
        let config = ctx.config().snapshot();
        let storage_root = self
            .edit
            .as_ref()
            .map(TextEdit::cursor_display)
            .unwrap_or_else(|| config.storage_root.display().to_string());
        let rows = vec![
            pane::row(
                0,
                self.selected.index(),
                &t!("settings.language"),
                current_language_name(),
                theme,
            ),
            pane::row(
                1,
                self.selected.index(),
                &t!("settings.storage_root"),
                storage_root,
                theme,
            ),
        ];
        pane::render_rows(frame, areas.body, rows, &mut self.selected, theme);
        let anchor = pane::row_anchor(areas.body, self.selected.index());
        if let Some(popup) = &mut self.language {
            popup.set_anchor(anchor);
            popup.render(ctx, frame, areas.full);
        }
        if let Some(popup) = &mut self.picker {
            popup.render(ctx, frame, areas.full);
        }
    }

    fn on_input(&mut self, ctx: &Services, input: InputEvent) -> PaneEvent {
        if let Some(popup) = &mut self.language {
            return match popup.on_input(ctx, input) {
                ModalOutcome::Ignored => PaneEvent::Ignored,
                ModalOutcome::Handled => PaneEvent::Handled,
                ModalOutcome::Cancelled => {
                    self.language = None;
                    PaneEvent::Handled
                }
                ModalOutcome::Completed(code) => {
                    self.language = None;
                    self.select_language(ctx, code);
                    PaneEvent::Handled
                }
            };
        }

        if let Some(popup) = &mut self.picker {
            return match popup.on_input(ctx, input) {
                ModalOutcome::Ignored => PaneEvent::Ignored,
                ModalOutcome::Handled => PaneEvent::Handled,
                ModalOutcome::Cancelled => {
                    self.picker = None;
                    PaneEvent::Handled
                }
                ModalOutcome::Completed(path) => {
                    self.picker = None;
                    self.set_storage_root(ctx, path);
                    PaneEvent::Handled
                }
            };
        }

        if self.edit.is_some() {
            return self.on_edit_input(ctx, input);
        }

        match self.selected.handle_input(&input) {
            ListAction::Changed => return PaneEvent::Handled,
            ListAction::Activate(_) => {
                self.activate(ctx);
                return PaneEvent::Handled;
            }
            ListAction::Ignored => {}
        }

        let Some(key) = input.key() else {
            return PaneEvent::Ignored;
        };
        match key.code {
            KeyCode::Char('e' | 'E')
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if self.selected.index() == 1 {
                    self.start_path_edit(&ctx.config().snapshot());
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
            return popup.keybindings();
        }
        if let Some(popup) = &self.picker {
            return popup.keybindings();
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
        match self.selected.index() {
            0 => self.open_language(),
            1 => self.open_picker(ctx.config().snapshot().storage_root),
            _ => {}
        }
    }

    fn adjust(&mut self, ctx: &Services, forward: bool) {
        if self.selected.index() != 0 {
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
            FilePickerOptions {
                cwd: start,
                preferred_extensions: Vec::new(),
            },
            FilePickerMode::Directory,
            t!("settings.pick_directory"),
        ));
    }

    fn start_path_edit(&mut self, config: &LauncherConfig) {
        self.error = None;
        self.edit = Some(TextEdit::new(config.storage_root.display().to_string()));
    }

    fn on_edit_input(&mut self, ctx: &Services, input: InputEvent) -> PaneEvent {
        let event = match self.edit.as_mut() {
            Some(edit) => edit.handle_input(&input),
            None => return PaneEvent::Ignored,
        };

        match event {
            EditEvent::Submit => {
                let Some(edit) = self.edit.take() else {
                    return PaneEvent::Handled;
                };
                let value = edit.input().trim();
                if value.is_empty() {
                    self.error = Some(t!("settings.path_empty").into_owned());
                    return PaneEvent::Handled;
                }

                let mut config = ctx.config().snapshot();
                config.storage_root = PathBuf::from(value);
                self.apply(ctx, config);
                PaneEvent::Handled
            }
            EditEvent::Cancel => {
                self.edit = None;
                self.error = None;
                PaneEvent::Handled
            }
            EditEvent::Changed => PaneEvent::Handled,
            EditEvent::Unhandled => PaneEvent::Ignored,
        }
    }

    fn select_language(&mut self, ctx: &Services, code: String) {
        if let Err(error) = i18n::set_locale(&code) {
            self.error = Some(format!("{error:#}"));
            return;
        }

        let mut config = ctx.config().snapshot();
        config.language = Some(i18n::current_locale());
        ctx.redraw().trigger();
        self.apply(ctx, config);
    }

    fn set_storage_root(&mut self, ctx: &Services, path: PathBuf) {
        let mut config = ctx.config().snapshot();
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
        match self.selected.index() {
            0 => RowHint::Select,
            _ => RowHint::Browse,
        }
    }
}

fn current_language_name() -> String {
    let current = i18n::current_locale();
    i18n::available_locales()
        .iter()
        .find(|locale| locale.code == current)
        .map(|locale| locale.name.clone())
        .unwrap_or(current)
}
