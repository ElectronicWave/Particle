use crate::t;
use elemental::core::auth::accounts::AccountId;
use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
    widgets::ListState,
};

use crate::app::config::microsoft_client_id;
use crate::pages::content::{Content, ContentEvent};
use crate::services::Services;
use crate::widgets::{
    dropdown::Dropdown,
    popup::{Popup, PopupEvent},
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

#[derive(Debug)]
enum Mode {
    List,
    AddOffline(TextEdit),
    Yggdrasil(YggdrasilForm),
    Authorize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthorizerKind {
    Microsoft,
    Yggdrasil,
    Offline,
}

#[derive(Debug)]
pub struct AccountsContent {
    mode: Mode,
    authorizer: Option<Dropdown<AuthorizerKind>>,
    list: ListState,
    error: Option<String>,
}

impl Default for AccountsContent {
    fn default() -> Self {
        Self {
            mode: Mode::List,
            authorizer: None,
            list: ListState::default().with_selected(Some(0)),
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
    }

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> ContentEvent {
        if let Some(popup) = &mut self.authorizer {
            match popup.handle_key(key) {
                PopupEvent::Ignored | PopupEvent::Handled => return ContentEvent::Handled,
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

        match std::mem::replace(&mut self.mode, Mode::List) {
            Mode::List => self.on_list_key(ctx, key),
            Mode::Authorize => {
                if key.code == KeyCode::Esc {
                    ctx.accounts().cancel_authorization();
                }
                ContentEvent::Handled
            }
            Mode::AddOffline(mut edit) => {
                match edit.handle_key(key) {
                    EditEvent::Submit => {
                        let name = edit.input().trim().to_owned();
                        if !name.is_empty() {
                            ctx.accounts().sign_in_offline(name);
                        }
                    }
                    EditEvent::Changed | EditEvent::Unhandled => {
                        self.mode = Mode::AddOffline(edit);
                    }
                    EditEvent::Cancel => {}
                }
                ContentEvent::Handled
            }
            Mode::Yggdrasil(mut form) => {
                match form.handle_key(key) {
                    YggdrasilFormEvent::Ignored | YggdrasilFormEvent::Changed => {
                        self.error = None;
                        self.mode = Mode::Yggdrasil(form);
                    }
                    YggdrasilFormEvent::Cancel => {}
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
                    }
                }
                ContentEvent::Handled
            }
        }
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        if let Some(popup) = &self.authorizer {
            return popup.hints();
        }
        match self.mode {
            Mode::List => list_hints(),
            Mode::AddOffline(_) => add_hints(),
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
            KeyCode::Down => {
                self.list.select_next();
                ContentEvent::Handled
            }
            KeyCode::Up => {
                self.list.select_previous();
                ContentEvent::Handled
            }
            KeyCode::Enter => match self.selected_account(ctx) {
                Some(account_id) => {
                    ctx.accounts().select(account_id);
                    ContentEvent::Handled
                }
                None => ContentEvent::Handled,
            },
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
                    (t!("accounts.offline").into_owned(), AuthorizerKind::Offline),
                ];
                self.authorizer =
                    Some(Dropdown::open(t!("accounts.authorizer_picker"), options, 0));
                ContentEvent::Handled
            }
            KeyCode::Char('d') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(account_id) = self.selected_account(ctx) {
                    ctx.accounts().remove(account_id);
                }
                ContentEvent::Handled
            }
            KeyCode::Char('r') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                ctx.accounts().load();
                ContentEvent::Handled
            }
            _ => ContentEvent::Ignored,
        }
    }

    fn selected_account(&self, ctx: &Services) -> Option<AccountId> {
        let snapshot = ctx.accounts().state().snapshot();
        let state = snapshot.view().data?;
        let selected = self.list.selected()?;
        state.accounts.get(selected).map(|account| account.id)
    }
}
