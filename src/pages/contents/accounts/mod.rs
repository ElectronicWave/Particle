use std::path::{Path, PathBuf};

use crate::t;
use elemental::{
    core::auth::{accounts::AccountId, texture::SkinModel},
    launcher::{CapeId, SkinId},
};
use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
};

use crate::app::config::microsoft_client_id;
use crate::pages::content::{Content, ContentEvent};
use crate::services::Services;
use crate::widgets::{
    confirm::Confirmation,
    dropdown::Dropdown,
    filepicker::{FilePicker, FilePickerMode, FilePickerOptions},
    list::ListSelection,
    png::PngImage,
    popup::{Popup, PopupEvent},
    skin::SkinView,
    text_edit::{EditEvent, TextEdit},
};

mod view;
mod yggdrasil;

use yggdrasil::{
    YggdrasilForm, YggdrasilFormEvent, hints as yggdrasil_hints, validate as validate_yggdrasil,
};

fn list_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("↑/↓", t!("common.browse")),
        KeyHint::new("ENTER", t!("common.select")),
        KeyHint::new("A", t!("common.add_account")),
        KeyHint::new("S", t!("accounts.skins")),
        KeyHint::new("C", t!("accounts.capes")),
        KeyHint::new("D", t!("common.remove")),
        KeyHint::new("R", t!("common.refresh")),
        KeyHint::new("ESC", t!("common.return")),
    ]
}

fn add_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("ENTER", t!("common.sign_in")),
        KeyHint::new("ESC", t!("common.cancel")),
    ]
}

fn skin_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("↑/↓", t!("common.browse")),
        KeyHint::new("ENTER", t!("accounts.skin_select")),
        KeyHint::new("I", t!("accounts.skin_import")),
        KeyHint::new("M", t!("accounts.skin_model")),
        KeyHint::new("R", t!("accounts.rename")),
        KeyHint::new("C", t!("accounts.skin_clear")),
        KeyHint::new("D", t!("common.remove")),
        KeyHint::new("ESC", t!("common.return")),
    ]
}

fn cape_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("↑/↓", t!("common.browse")),
        KeyHint::new("ENTER", t!("accounts.cape_select")),
        KeyHint::new("I", t!("accounts.cape_import")),
        KeyHint::new("R", t!("accounts.rename")),
        KeyHint::new("C", t!("accounts.cape_clear")),
        KeyHint::new("D", t!("common.remove")),
        KeyHint::new("ESC", t!("common.return")),
    ]
}

#[derive(Debug)]
enum Mode {
    List,
    AddOffline(TextEdit),
    AddLocalYggdrasil(TextEdit),
    Yggdrasil(YggdrasilForm),
    Authorize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthorizerKind {
    Microsoft,
    Yggdrasil,
    LocalYggdrasil,
    Offline,
}

#[derive(Debug)]
struct SkinMode {
    account_id: AccountId,
    list: ListSelection,
    model: SkinModel,
    view: SkinView,
    picker: Option<FilePicker>,
    import: Option<ImportPrompt>,
    rename: Option<SkinRename>,
    preview: Option<SkinPreviewSlot>,
}

#[derive(Debug)]
struct CapeMode {
    account_id: AccountId,
    list: ListSelection,
    picker: Option<FilePicker>,
    import: Option<ImportPrompt>,
    rename: Option<CapeRename>,
    preview: Option<CapePreviewSlot>,
}

#[derive(Debug)]
struct ImportPrompt {
    path: PathBuf,
    edit: TextEdit,
    error: Option<String>,
}

#[derive(Debug)]
struct SkinRename {
    skin_id: SkinId,
    edit: TextEdit,
    error: Option<String>,
}

#[derive(Debug)]
struct CapeRename {
    cape_id: CapeId,
    edit: TextEdit,
    error: Option<String>,
}

#[derive(Debug)]
enum DeleteTarget {
    Account(AccountId),
    Skin {
        account_id: AccountId,
        skin_id: SkinId,
    },
    Cape {
        account_id: AccountId,
        cape_id: CapeId,
    },
}

#[derive(Debug)]
struct SkinPreviewSlot {
    path: PathBuf,
    model: SkinModel,
    view: SkinView,
    image: Result<PngImage, String>,
}

#[derive(Debug)]
struct CapePreviewSlot {
    path: PathBuf,
    image: Result<PngImage, String>,
}

#[derive(Debug)]
pub struct AccountsContent {
    mode: Mode,
    authorizer: Option<Dropdown<AuthorizerKind>>,
    skins: Option<SkinMode>,
    capes: Option<CapeMode>,
    confirm: Option<Confirmation<DeleteTarget>>,
    list: ListSelection,
    error: Option<String>,
}

impl Default for AccountsContent {
    fn default() -> Self {
        Self {
            mode: Mode::List,
            authorizer: None,
            skins: None,
            capes: None,
            confirm: None,
            list: ListSelection::wrap(0),
            error: None,
        }
    }
}

impl Content for AccountsContent {
    fn init(&mut self, ctx: &Services) {
        ctx.accounts().load();
    }

    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect) {
        view::render(self, ctx, frame, area);

        if let Some(confirm) = &self.confirm {
            let anchor = Rect {
                x: area.x,
                y: area.bottom().saturating_sub(1),
                width: 1,
                height: 1,
            };
            confirm.render(frame, area, anchor, ctx.ui().theme());
        }
    }

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> ContentEvent {
        if let Some(confirm) = &mut self.confirm {
            match confirm.handle_key(key) {
                PopupEvent::Ignored => return ContentEvent::Ignored,
                PopupEvent::Handled => return ContentEvent::Handled,
                PopupEvent::Cancelled => {
                    self.confirm = None;
                    return ContentEvent::Handled;
                }
                PopupEvent::Completed(target) => {
                    self.confirm = None;
                    self.apply_delete(ctx, target);
                    return ContentEvent::Handled;
                }
            }
        }

        if let Some(popup) = &mut self.authorizer {
            match popup.handle_key(key) {
                PopupEvent::Ignored => return ContentEvent::Ignored,
                PopupEvent::Handled => return ContentEvent::Handled,
                PopupEvent::Cancelled => {
                    self.authorizer = None;
                    return ContentEvent::Handled;
                }
                PopupEvent::Completed(kind) => {
                    self.authorizer = None;
                    self.start_authorizer(ctx, kind);
                    return ContentEvent::Handled;
                }
            }
        }

        if let Some(skin_mode) = &mut self.skins {
            if let Some(picker) = &mut skin_mode.picker {
                match picker.handle_key(key) {
                    PopupEvent::Ignored => return ContentEvent::Ignored,
                    PopupEvent::Handled => return ContentEvent::Handled,
                    PopupEvent::Cancelled => {
                        skin_mode.picker = None;
                        return ContentEvent::Handled;
                    }
                    PopupEvent::Completed(path) => {
                        let name = default_texture_name(&path);
                        skin_mode.import = Some(ImportPrompt {
                            path,
                            edit: TextEdit::new(name),
                            error: None,
                        });
                        skin_mode.picker = None;
                        return ContentEvent::Handled;
                    }
                }
            }

            if let Some(import) = &mut skin_mode.import {
                match import.edit.handle_key(key) {
                    EditEvent::Submit => {
                        let name = import.edit.input().trim().to_owned();
                        if name.is_empty() {
                            import.error = Some(t!("accounts.skin_name_required").into_owned());
                        } else {
                            let path = import.path.clone();
                            skin_mode.import = None;
                            ctx.skins()
                                .import(skin_mode.account_id, name, path, skin_mode.model);
                        }
                    }
                    EditEvent::Changed => import.error = None,
                    EditEvent::Cancel => skin_mode.import = None,
                    EditEvent::Unhandled => return ContentEvent::Ignored,
                }
                return ContentEvent::Handled;
            }

            if let Some(rename) = &mut skin_mode.rename {
                match rename.edit.handle_key(key) {
                    EditEvent::Submit => {
                        let name = rename.edit.input().trim().to_owned();
                        if name.is_empty() {
                            rename.error = Some(t!("accounts.name_required").into_owned());
                        } else {
                            let skin_id = rename.skin_id;
                            skin_mode.rename = None;
                            ctx.skins().rename(skin_mode.account_id, skin_id, name);
                        }
                    }
                    EditEvent::Changed => rename.error = None,
                    EditEvent::Cancel => skin_mode.rename = None,
                    EditEvent::Unhandled => return ContentEvent::Ignored,
                }
                return ContentEvent::Handled;
            }

            match key.code {
                KeyCode::Down | KeyCode::Up | KeyCode::Home | KeyCode::End => {
                    skin_mode.list.handle_key(key.code);
                }
                KeyCode::Char('m') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    skin_mode.model = match skin_mode.model {
                        SkinModel::Classic => SkinModel::Slim,
                        SkinModel::Slim => SkinModel::Classic,
                    };
                }
                KeyCode::Char('i') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                    skin_mode.picker = Some(FilePicker::open(
                        FilePickerOptions {
                            cwd,
                            preferred_extensions: vec!["png".to_owned()],
                        },
                        FilePickerMode::File,
                        t!("accounts.skin_pick"),
                    ));
                }
                KeyCode::Char('c') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    ctx.skins().clear(skin_mode.account_id);
                }
                KeyCode::Char('r') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if let Some((skin_id, name)) = selected_skin_entry(ctx, skin_mode) {
                        skin_mode.rename = Some(SkinRename {
                            skin_id,
                            edit: TextEdit::new(name),
                            error: None,
                        });
                    }
                }
                KeyCode::Char('d') | KeyCode::Delete
                    if !key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    if let Some((skin_id, name)) = selected_skin_entry(ctx, skin_mode) {
                        self.confirm = Some(Confirmation::open(
                            t!("accounts.confirm_remove_skin", skin = name).into_owned(),
                            DeleteTarget::Skin {
                                account_id: skin_mode.account_id,
                                skin_id,
                            },
                        ));
                    }
                }
                KeyCode::Enter => {
                    if let Some(skin_id) = selected_skin(ctx, skin_mode) {
                        ctx.skins()
                            .choose(skin_mode.account_id, skin_id, skin_mode.model);
                    }
                }
                KeyCode::Esc => self.skins = None,
                _ => return ContentEvent::Ignored,
            }
            return ContentEvent::Handled;
        }

        if let Some(cape_mode) = &mut self.capes {
            if let Some(picker) = &mut cape_mode.picker {
                match picker.handle_key(key) {
                    PopupEvent::Ignored => return ContentEvent::Ignored,
                    PopupEvent::Handled => return ContentEvent::Handled,
                    PopupEvent::Cancelled => {
                        cape_mode.picker = None;
                        return ContentEvent::Handled;
                    }
                    PopupEvent::Completed(path) => {
                        let name = default_texture_name(&path);
                        cape_mode.import = Some(ImportPrompt {
                            path,
                            edit: TextEdit::new(name),
                            error: None,
                        });
                        cape_mode.picker = None;
                        return ContentEvent::Handled;
                    }
                }
            }

            if let Some(import) = &mut cape_mode.import {
                match import.edit.handle_key(key) {
                    EditEvent::Submit => {
                        let name = import.edit.input().trim().to_owned();
                        if name.is_empty() {
                            import.error = Some(t!("accounts.name_required").into_owned());
                        } else {
                            let path = import.path.clone();
                            cape_mode.import = None;
                            ctx.capes().import(cape_mode.account_id, name, path);
                        }
                    }
                    EditEvent::Changed => import.error = None,
                    EditEvent::Cancel => cape_mode.import = None,
                    EditEvent::Unhandled => return ContentEvent::Ignored,
                }
                return ContentEvent::Handled;
            }

            if let Some(rename) = &mut cape_mode.rename {
                match rename.edit.handle_key(key) {
                    EditEvent::Submit => {
                        let name = rename.edit.input().trim().to_owned();
                        if name.is_empty() {
                            rename.error = Some(t!("accounts.name_required").into_owned());
                        } else {
                            let cape_id = rename.cape_id;
                            cape_mode.rename = None;
                            ctx.capes().rename(cape_mode.account_id, cape_id, name);
                        }
                    }
                    EditEvent::Changed => rename.error = None,
                    EditEvent::Cancel => cape_mode.rename = None,
                    EditEvent::Unhandled => return ContentEvent::Ignored,
                }
                return ContentEvent::Handled;
            }

            match key.code {
                KeyCode::Down | KeyCode::Up | KeyCode::Home | KeyCode::End => {
                    cape_mode.list.handle_key(key.code);
                }
                KeyCode::Char('i') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                    cape_mode.picker = Some(FilePicker::open(
                        FilePickerOptions {
                            cwd,
                            preferred_extensions: vec!["png".to_owned()],
                        },
                        FilePickerMode::File,
                        t!("accounts.cape_pick"),
                    ));
                }
                KeyCode::Char('c') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    ctx.capes().clear(cape_mode.account_id);
                }
                KeyCode::Char('r') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if let Some((cape_id, name)) = selected_cape_entry(ctx, cape_mode) {
                        cape_mode.rename = Some(CapeRename {
                            cape_id,
                            edit: TextEdit::new(name),
                            error: None,
                        });
                    }
                }
                KeyCode::Char('d') | KeyCode::Delete
                    if !key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    if let Some((cape_id, name)) = selected_cape_entry(ctx, cape_mode) {
                        self.confirm = Some(Confirmation::open(
                            t!("accounts.confirm_remove_cape", cape = name).into_owned(),
                            DeleteTarget::Cape {
                                account_id: cape_mode.account_id,
                                cape_id,
                            },
                        ));
                    }
                }
                KeyCode::Enter => {
                    if let Some(cape_id) = selected_cape(ctx, cape_mode) {
                        ctx.capes().choose(cape_mode.account_id, cape_id);
                    }
                }
                KeyCode::Esc => self.capes = None,
                _ => return ContentEvent::Ignored,
            }
            return ContentEvent::Handled;
        }

        match std::mem::replace(&mut self.mode, Mode::List) {
            Mode::List => self.on_list_key(ctx, key),
            Mode::Authorize => {
                if key.code == KeyCode::Esc {
                    ctx.accounts().cancel_authorization();
                    return ContentEvent::Handled;
                }
                self.mode = Mode::Authorize;
                ContentEvent::Ignored
            }
            Mode::AddOffline(mut edit) => match edit.handle_key(key) {
                EditEvent::Submit => {
                    let name = edit.input().trim().to_owned();
                    if !name.is_empty() {
                        ctx.accounts().sign_in_offline(name);
                    }
                    ContentEvent::Handled
                }
                EditEvent::Changed => {
                    self.mode = Mode::AddOffline(edit);
                    ContentEvent::Handled
                }
                EditEvent::Unhandled => {
                    self.mode = Mode::AddOffline(edit);
                    ContentEvent::Ignored
                }
                EditEvent::Cancel => ContentEvent::Handled,
            },
            Mode::AddLocalYggdrasil(mut edit) => match edit.handle_key(key) {
                EditEvent::Submit => {
                    let name = edit.input().trim().to_owned();
                    if !name.is_empty() {
                        ctx.accounts().sign_in_local_yggdrasil(name);
                    }
                    ContentEvent::Handled
                }
                EditEvent::Changed => {
                    self.mode = Mode::AddLocalYggdrasil(edit);
                    ContentEvent::Handled
                }
                EditEvent::Unhandled => {
                    self.mode = Mode::AddLocalYggdrasil(edit);
                    ContentEvent::Ignored
                }
                EditEvent::Cancel => ContentEvent::Handled,
            },
            Mode::Yggdrasil(mut form) => match form.handle_key(key) {
                YggdrasilFormEvent::Ignored => {
                    self.error = None;
                    self.mode = Mode::Yggdrasil(form);
                    ContentEvent::Ignored
                }
                YggdrasilFormEvent::Changed => {
                    self.error = None;
                    self.mode = Mode::Yggdrasil(form);
                    ContentEvent::Handled
                }
                YggdrasilFormEvent::Cancel => ContentEvent::Handled,
                YggdrasilFormEvent::Submit => {
                    let (base_url, username, password) = form.values();
                    if let Some(error) = validate_yggdrasil(&base_url, &username, &password) {
                        self.error = Some(error);
                        self.mode = Mode::Yggdrasil(form);
                    } else if ctx
                        .accounts()
                        .sign_in_yggdrasil(base_url, username, password)
                    {
                        self.error = None;
                    } else {
                        self.error = Some(t!("accounts.busy").into_owned());
                        self.mode = Mode::Yggdrasil(form);
                    }
                    ContentEvent::Handled
                }
            },
        }
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        if let Some(confirm) = &self.confirm {
            return confirm.hints();
        }
        if let Some(skin_mode) = &self.skins {
            if let Some(picker) = &skin_mode.picker {
                return picker.hints();
            }
            if skin_mode.import.is_some() {
                return vec![
                    KeyHint::new("ENTER", t!("accounts.confirm")),
                    KeyHint::new("ESC", t!("common.cancel")),
                ];
            }
            if skin_mode.rename.is_some() {
                return vec![
                    KeyHint::new("ENTER", t!("accounts.confirm")),
                    KeyHint::new("ESC", t!("common.cancel")),
                ];
            }
            return skin_hints();
        }
        if let Some(cape_mode) = &self.capes {
            if let Some(picker) = &cape_mode.picker {
                return picker.hints();
            }
            if cape_mode.import.is_some() || cape_mode.rename.is_some() {
                return vec![
                    KeyHint::new("ENTER", t!("accounts.confirm")),
                    KeyHint::new("ESC", t!("common.cancel")),
                ];
            }
            return cape_hints();
        }
        if let Some(popup) = &self.authorizer {
            return popup.hints();
        }
        match self.mode {
            Mode::List => list_hints(),
            Mode::AddOffline(_) => add_hints(),
            Mode::AddLocalYggdrasil(_) => add_hints(),
            Mode::Yggdrasil(_) => yggdrasil_hints(),
            Mode::Authorize => vec![KeyHint::new("ESC", t!("common.cancel"))],
        }
    }
}

impl AccountsContent {
    fn start_authorizer(&mut self, ctx: &Services, kind: AuthorizerKind) {
        self.error = None;
        match kind {
            AuthorizerKind::Offline => {
                self.mode = Mode::AddOffline(TextEdit::new(String::new()));
            }
            AuthorizerKind::Yggdrasil => {
                self.mode = Mode::Yggdrasil(YggdrasilForm::new());
            }
            AuthorizerKind::LocalYggdrasil => {
                self.mode = Mode::AddLocalYggdrasil(TextEdit::new(String::new()));
            }
            AuthorizerKind::Microsoft => {
                let Some(client_id) = microsoft_client_id().map(str::to_owned) else {
                    self.error = Some(t!("accounts.microsoft_not_configured").into_owned());
                    return;
                };
                if ctx.accounts().sign_in_microsoft(client_id) {
                    self.mode = Mode::Authorize;
                } else {
                    self.error = Some(t!("accounts.busy").into_owned());
                }
            }
        }
    }

    fn on_list_key(&mut self, ctx: &Services, key: KeyEvent) -> ContentEvent {
        match key.code {
            KeyCode::Down | KeyCode::Up | KeyCode::Home | KeyCode::End => {
                self.list.handle_key(key.code);
                ContentEvent::Handled
            }
            KeyCode::Enter => {
                if let Some(account_id) = self.selected_account(ctx) {
                    ctx.accounts().select(account_id);
                }
                ContentEvent::Handled
            }
            KeyCode::Char('a') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.error = None;
                let options = vec![
                    (
                        t!("accounts.microsoft").into_owned(),
                        AuthorizerKind::Microsoft,
                    ),
                    (
                        t!("accounts.yggdrasil").into_owned(),
                        AuthorizerKind::Yggdrasil,
                    ),
                    (
                        t!("accounts.local_yggdrasil").into_owned(),
                        AuthorizerKind::LocalYggdrasil,
                    ),
                    (t!("accounts.offline").into_owned(), AuthorizerKind::Offline),
                ];
                self.authorizer =
                    Some(Dropdown::open(t!("accounts.authorizer_picker"), options, 0));
                ContentEvent::Handled
            }
            KeyCode::Char('d') | KeyCode::Delete
                if !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(account) = self.selected_summary(ctx) {
                    self.confirm = Some(Confirmation::open(
                        t!("accounts.confirm_remove_account", account = account.name).into_owned(),
                        DeleteTarget::Account(account.id),
                    ));
                }
                ContentEvent::Handled
            }
            KeyCode::Char('r') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                ctx.accounts().load();
                ContentEvent::Handled
            }
            KeyCode::Char('s') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(account_id) = self.selected_account(ctx) {
                    let snapshot = ctx.skins().snapshot();
                    let model = snapshot
                        .view()
                        .data
                        .filter(|state| state.account_id == account_id)
                        .and_then(|state| state.selected.as_ref())
                        .map(|selected| selected.model)
                        .unwrap_or(SkinModel::Classic);
                    ctx.skins().load(account_id);
                    self.skins = Some(SkinMode {
                        account_id,
                        list: ListSelection::wrap(0),
                        model,
                        view: SkinView::Full,
                        picker: None,
                        import: None,
                        rename: None,
                        preview: None,
                    });
                    self.capes = None;
                }
                ContentEvent::Handled
            }
            KeyCode::Char('c') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(account_id) = self.selected_account(ctx) {
                    ctx.capes().load(account_id);
                    self.capes = Some(CapeMode {
                        account_id,
                        list: ListSelection::wrap(0),
                        picker: None,
                        import: None,
                        rename: None,
                        preview: None,
                    });
                    self.skins = None;
                }
                ContentEvent::Handled
            }
            _ => ContentEvent::Ignored,
        }
    }

    fn selected_account(&self, ctx: &Services) -> Option<AccountId> {
        self.selected_summary(ctx).map(|account| account.id)
    }

    fn selected_summary(&self, ctx: &Services) -> Option<crate::services::AccountSummary> {
        let snapshot = ctx.accounts().state().snapshot();
        let state = snapshot.view().data?;
        let selected = self.list.selected()?;
        state.accounts.get(selected).cloned()
    }

    fn apply_delete(&self, ctx: &Services, target: DeleteTarget) {
        match target {
            DeleteTarget::Account(account_id) => {
                ctx.accounts().remove(account_id);
            }
            DeleteTarget::Skin {
                account_id,
                skin_id,
            } => {
                ctx.skins().remove(account_id, skin_id);
            }
            DeleteTarget::Cape {
                account_id,
                cape_id,
            } => {
                ctx.capes().remove(account_id, cape_id);
            }
        }
    }
}

fn selected_skin(ctx: &Services, skin_mode: &SkinMode) -> Option<SkinId> {
    selected_skin_entry(ctx, skin_mode).map(|(skin_id, _)| skin_id)
}

fn selected_skin_entry(ctx: &Services, skin_mode: &SkinMode) -> Option<(SkinId, String)> {
    let snapshot = ctx.skins().snapshot();
    let state = snapshot.view().data?;
    if state.account_id != skin_mode.account_id {
        return None;
    }
    let selected = skin_mode.list.selected()?;
    state
        .entries
        .get(selected)
        .map(|entry| (entry.asset.id, entry.asset.name.clone()))
}

fn selected_cape(ctx: &Services, cape_mode: &CapeMode) -> Option<CapeId> {
    selected_cape_entry(ctx, cape_mode).map(|(cape_id, _)| cape_id)
}

fn selected_cape_entry(ctx: &Services, cape_mode: &CapeMode) -> Option<(CapeId, String)> {
    let snapshot = ctx.capes().snapshot();
    let state = snapshot.view().data?;
    if state.account_id != cape_mode.account_id {
        return None;
    }
    let selected = cape_mode.list.selected()?;
    state
        .entries
        .get(selected)
        .map(|entry| (entry.asset.id, entry.asset.name.clone()))
}

fn default_texture_name(path: &Path) -> String {
    let Some(stem) = path.file_stem() else {
        return String::new();
    };
    let stem = stem.to_string_lossy();
    if looks_hashed(&stem) {
        String::new()
    } else {
        stem.into_owned()
    }
}

fn looks_hashed(name: &str) -> bool {
    let compact = name
        .chars()
        .filter(|character| *character != '-')
        .collect::<String>();
    matches!(compact.len(), 32 | 40 | 64)
        && compact
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}
