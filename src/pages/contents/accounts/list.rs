use std::borrow::Cow;

use elemental::core::auth::{
    accounts::AccountId,
    authorizers::{
        microsoft::MICROSOFT_AUTHORIZER_ID, offline::OFFLINE_AUTHORIZER_ID,
        yggdrasil::YGGDRASIL_AUTHORIZER_ID,
    },
    texture::TextureType,
};
use ratail::{
    KeyChord, KeyHint, KeyMap, ModalHost, NavAction, Page, PageEvent, SnapshotStatus, WithModal,
};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
};

use crate::pages::content::{render_loading, render_message, render_snapshot};
use crate::services::{
    AccountPreview, AccountState, AccountSummary, AuthorizationState, PreviewSource, Services,
};
use crate::t;
use crate::theme::Theme;
use crate::widgets::{confirm::Confirmation, dropdown::Dropdown};

use super::{
    AccountRoute, AuthorizerKind, NamePrompt, bind_list_nav, bind_remove_and_back,
    render_preview_notice, skin_model_label,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AccountCapability {
    skin: bool,
    cape: bool,
}

impl Default for AccountCapability {
    fn default() -> Self {
        Self {
            skin: true,
            cape: true,
        }
    }
}

pub(super) struct AccountsListPage {
    list: crate::widgets::list::ListSelection,
    capability: AccountCapability,
    error: Option<String>,
    keys: KeyMap<ListAction>,
}

#[derive(Debug, Clone, Copy)]
enum ListAction {
    Navigate(KeyCode),
    Select,
    Add,
    Edit,
    Remove,
    Refresh,
    Skins,
    Capes,
    Back,
}

impl Default for AccountsListPage {
    fn default() -> Self {
        Self {
            list: crate::widgets::list::ListSelection::wrap(0),
            capability: AccountCapability::default(),
            error: None,
            keys: list_keys(AccountCapability::default()),
        }
    }
}

impl AccountsListPage {
    fn sync(&mut self, ctx: &Services) {
        let snapshot = ctx.accounts().state().snapshot();
        if let Some(account) = snapshot
            .view()
            .data
            .and_then(|state| state.accounts.get(self.list.index()))
        {
            ctx.skins().preview(account.id);
        }
        let capability = self
            .selected_summary(ctx)
            .map(|account| account_capability(ctx, &account))
            .unwrap_or_default();
        if capability != self.capability {
            self.capability = capability;
            self.keys = list_keys(capability);
        }
    }

    fn start_authorizer(
        &mut self,
        ctx: &Services,
        modals: &mut ModalHost<Services, Self, AccountRoute>,
        kind: AuthorizerKind,
    ) -> PageEvent<AccountRoute> {
        self.error = None;
        match kind {
            AuthorizerKind::Offline => {
                prompt_username(
                    modals,
                    String::new(),
                    t!("accounts.offline_help"),
                    |_, ctx, name| ctx.accounts().sign_in_offline(name),
                );
                PageEvent::Handled
            }
            AuthorizerKind::Yggdrasil => {
                PageEvent::Navigate(NavAction::Push(AccountRoute::AddYggdrasil))
            }
            AuthorizerKind::LocalYggdrasil => {
                prompt_username(
                    modals,
                    String::new(),
                    t!("accounts.local_yggdrasil_help"),
                    |_, ctx, name| ctx.accounts().sign_in_local_yggdrasil(name),
                );
                PageEvent::Handled
            }
            AuthorizerKind::Microsoft => {
                let Some(client_id) = crate::app::config::microsoft_client_id().map(str::to_owned)
                else {
                    self.error = Some(t!("accounts.microsoft_not_configured").into_owned());
                    return PageEvent::Handled;
                };
                if ctx.accounts().sign_in_microsoft(client_id) {
                    PageEvent::Navigate(NavAction::Push(AccountRoute::MicrosoftAuth))
                } else {
                    self.error = Some(t!("accounts.busy").into_owned());
                    PageEvent::Handled
                }
            }
        }
    }

    fn selected_summary(&self, ctx: &Services) -> Option<AccountSummary> {
        let snapshot = ctx.accounts().state().snapshot();
        let state = snapshot.view().data?;
        state.accounts.get(self.list.selected()?).cloned()
    }

    fn handle_key(
        &mut self,
        ctx: &Services,
        modals: &mut ModalHost<Services, Self, AccountRoute>,
        key: KeyEvent,
    ) -> PageEvent<AccountRoute> {
        let Some(action) = self.keys.action_for(&key) else {
            return PageEvent::Ignored;
        };

        match action {
            ListAction::Navigate(code) => {
                self.list.handle_key(code);
            }
            ListAction::Select => {
                if let Some(account) = self.selected_summary(ctx) {
                    ctx.accounts().select(account.id);
                }
            }
            ListAction::Add => self.open_authorizer_picker(modals),
            ListAction::Edit => return self.edit_account(ctx, modals),
            ListAction::Remove => self.confirm_remove_account(ctx, modals),
            ListAction::Refresh => {
                ctx.accounts().load();
            }
            ListAction::Skins => {
                return self.open_texture(
                    ctx,
                    |cap| cap.skin,
                    "accounts.skin_upload_unsupported",
                    AccountRoute::Skins,
                );
            }
            ListAction::Capes => {
                return self.open_texture(
                    ctx,
                    |cap| cap.cape,
                    "accounts.cape_upload_unsupported",
                    AccountRoute::Capes,
                );
            }
            ListAction::Back => return PageEvent::Navigate(NavAction::Pop),
        }

        PageEvent::Handled
    }

    fn open_authorizer_picker(&mut self, modals: &mut ModalHost<Services, Self, AccountRoute>) {
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
        modals.open(
            Dropdown::open(t!("accounts.authorizer_picker"), options, 0),
            |page, ctx, modals, kind| page.start_authorizer(ctx, modals, kind),
        );
    }

    fn confirm_remove_account(
        &self,
        ctx: &Services,
        modals: &mut ModalHost<Services, Self, AccountRoute>,
    ) {
        let Some(account) = self.selected_summary(ctx) else {
            return;
        };
        let title = t!("accounts.confirm_remove_account", account = account.name).into_owned();
        modals.open(
            Confirmation::open(title, account.id),
            |_page, ctx, _modals, id| {
                ctx.accounts().remove(id);
            },
        );
    }

    fn edit_account(
        &mut self,
        ctx: &Services,
        modals: &mut ModalHost<Services, Self, AccountRoute>,
    ) -> PageEvent<AccountRoute> {
        let Some(account) = self.selected_summary(ctx) else {
            return PageEvent::Handled;
        };
        self.error = None;

        if account.authorizer == OFFLINE_AUTHORIZER_ID {
            let id = account.id;
            prompt_username(
                modals,
                account.name,
                t!("accounts.rename_help"),
                move |_, ctx, name| ctx.accounts().edit_offline(id, name),
            );
            return PageEvent::Handled;
        }
        if account.authorizer == YGGDRASIL_AUTHORIZER_ID {
            if account.local {
                let id = account.id;
                prompt_username(
                    modals,
                    account.name,
                    t!("accounts.rename_help"),
                    move |_, ctx, name| ctx.accounts().edit_local_yggdrasil(id, name),
                );
                return PageEvent::Handled;
            }
            return PageEvent::Navigate(NavAction::Push(AccountRoute::EditYggdrasil {
                account_id: account.id,
                base_url: account.auth_server.unwrap_or_default(),
            }));
        }

        self.error = Some(t!("accounts.edit_unsupported").into_owned());
        PageEvent::Handled
    }

    fn open_texture(
        &mut self,
        ctx: &Services,
        allowed: fn(AccountCapability) -> bool,
        error_key: &'static str,
        route: fn(AccountId) -> AccountRoute,
    ) -> PageEvent<AccountRoute> {
        let Some(account) = self.selected_summary(ctx) else {
            return PageEvent::Handled;
        };
        if !allowed(account_capability(ctx, &account)) {
            self.error = Some(t!(error_key).into_owned());
            return PageEvent::Handled;
        }
        PageEvent::Navigate(NavAction::Push(route(account.id)))
    }

    fn render_body(&mut self, ctx: &Services, state: &AccountState, frame: &mut Frame, area: Rect) {
        let theme = ctx.ui().theme();
        self.list.set_len(state.accounts.len());
        if state.accounts.is_empty() {
            render_message(frame, area, t!("accounts.empty").as_ref());
            return;
        }

        let selected_account = self
            .list
            .selected()
            .and_then(|index| state.accounts.get(index));
        let preview_snapshot =
            selected_account.and_then(|account| ctx.skins().preview_snapshot(account.id));
        let has_skin = preview_snapshot.as_ref().is_some_and(|snapshot| {
            snapshot
                .view()
                .data
                .is_some_and(|preview| preview.image.is_some())
        });
        let (accounts_area, preview_area) = split_preview_area(area, has_skin);

        let items = state
            .accounts
            .iter()
            .map(|account| account_list_item(account, state.selected == Some(account.id), theme))
            .collect::<Vec<ListItem>>();
        let list = List::new(items).highlight_style(theme.selection_style());
        frame.render_stateful_widget(list, accounts_area, self.list.state_mut());

        if preview_area.width > 0
            && let (Some(account), Some(snapshot)) = (selected_account, preview_snapshot)
        {
            render_account_preview(account, &snapshot.view(), frame, preview_area, theme);
        }
    }
}

fn split_preview_area(area: Rect, has_skin: bool) -> (Rect, Rect) {
    if has_skin && area.width >= 46 {
        let width = (area.width / 3).clamp(18, 30);
        let [list, preview] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(width)]).areas(area);
        (list, preview)
    } else {
        (area, Rect::new(area.x, area.y, 0, 0))
    }
}

fn account_list_item(account: &AccountSummary, selected: bool, theme: Theme) -> ListItem<'static> {
    let marker = if selected { "*" } else { " " };
    let style = if selected {
        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)
    } else {
        Style::new()
    };
    ListItem::new(Line::from(vec![
        Span::styled(format!("{marker} {}", account.name), style),
        Span::styled(
            format!("  ({})", account.authorizer_label()),
            Style::new().fg(theme.text_dim),
        ),
    ]))
}

fn render_account_preview(
    account: &AccountSummary,
    view: &ratail::SnapshotView<'_, AccountPreview, anyhow::Error>,
    frame: &mut Frame,
    area: Rect,
    theme: Theme,
) {
    let [image_area, footer_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(area);
    match view.data {
        Some(preview) => {
            let Some(image) = &preview.image else {
                return;
            };
            frame.render_widget(image, image_area);
        }
        None if matches!(view.status, SnapshotStatus::Idle | SnapshotStatus::Loading) => {
            render_preview_notice(frame, image_area, t!("common.loading").as_ref(), theme);
        }
        None => {
            let message = match view.error {
                Some(error) => t!("common.error", error = error.as_ref()).into_owned(),
                None => t!("common.cancelled").into_owned(),
            };
            render_preview_notice(frame, image_area, &message, theme);
        }
    }

    let dim = Style::new().fg(theme.text_dim);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(account.authorizer_label(), dim),
            Line::styled(preview_detail(view.data.map(AsRef::as_ref)), dim),
        ]),
        footer_area,
    );
}

fn preview_detail(preview: Option<&AccountPreview>) -> String {
    let Some(preview) = preview.filter(|preview| preview.image.is_some()) else {
        return String::new();
    };
    let mut parts = Vec::new();
    if let Some(model) = preview.model {
        parts.push(skin_model_label(model).into_owned());
    }
    let source: Cow<'static, str> = match preview.source {
        PreviewSource::Profile => t!("accounts.preview_source_profile"),
        PreviewSource::Local => t!("accounts.preview_source_local"),
    };
    parts.push(source.into_owned());
    parts.join(" · ")
}

fn list_keys(capability: AccountCapability) -> KeyMap<ListAction> {
    let mut keys = KeyMap::new();
    bind_list_nav(
        &mut keys,
        ListAction::Navigate,
        ListAction::Select,
        t!("common.select"),
    );
    keys.bind_with_hint(
        KeyChord::char('a'),
        ListAction::Add,
        KeyHint::new("A", t!("common.add_account")),
    );
    keys.bind_with_hint(
        KeyChord::char('e'),
        ListAction::Edit,
        KeyHint::new("E", t!("common.edit")),
    );
    if capability.skin {
        keys.bind_with_hint(
            KeyChord::char('s'),
            ListAction::Skins,
            KeyHint::new("S", t!("accounts.skins")),
        );
    }
    if capability.cape {
        keys.bind_with_hint(
            KeyChord::char('c'),
            ListAction::Capes,
            KeyHint::new("C", t!("accounts.capes")),
        );
    }
    keys.bind_with_hint(
        KeyChord::char('r'),
        ListAction::Refresh,
        KeyHint::new("R", t!("common.refresh")),
    );
    bind_remove_and_back(&mut keys, Some(ListAction::Remove), ListAction::Back);
    keys
}

fn account_capability(ctx: &Services, account: &AccountSummary) -> AccountCapability {
    if account.authorizer == MICROSOFT_AUTHORIZER_ID
        || account.authorizer == OFFLINE_AUTHORIZER_ID
        || (account.authorizer == YGGDRASIL_AUTHORIZER_ID && account.local)
    {
        return AccountCapability::default();
    }

    let uploadable = ctx
        .skins()
        .preview_snapshot(account.id)
        .and_then(|snapshot| {
            snapshot
                .view()
                .data
                .and_then(|preview| preview.uploadable.clone())
        });
    match uploadable {
        Some(uploadable) => AccountCapability {
            skin: uploadable.contains(&TextureType::Skin),
            cape: uploadable.contains(&TextureType::Cape),
        },
        None => AccountCapability::default(),
    }
}

fn prompt_username<F>(
    modals: &mut ModalHost<Services, AccountsListPage, AccountRoute>,
    initial: String,
    help: Cow<'static, str>,
    submit: F,
) where
    F: FnOnce(&mut AccountsListPage, &Services, String) -> bool + 'static,
{
    modals.open(
        NamePrompt::new(
            t!("accounts.username"),
            initial,
            help,
            t!("accounts.name_required"),
        ),
        move |page, ctx, _modals, name| {
            if !submit(page, ctx, name) {
                page.error = Some(t!("accounts.busy").into_owned());
            }
        },
    );
}

impl WithModal<Services, AccountRoute> for AccountsListPage {
    fn init(
        &mut self,
        ctx: &Services,
        _route: &AccountRoute,
        _modals: &mut ratail::ModalHost<Services, Self, AccountRoute>,
    ) {
        ctx.accounts().load();
        self.sync(ctx);
    }

    fn resume(
        &mut self,
        ctx: &Services,
        _route: &AccountRoute,
        _modals: &mut ratail::ModalHost<Services, Self, AccountRoute>,
    ) {
        self.sync(ctx);
    }

    fn on_key(
        &mut self,
        ctx: &Services,
        _route: &AccountRoute,
        modals: &mut ratail::ModalHost<Services, Self, AccountRoute>,
        key: KeyEvent,
    ) -> ratail::PageEvent<AccountRoute> {
        let event = self.handle_key(ctx, modals, key);
        if event == PageEvent::Handled {
            self.sync(ctx);
        }
        event
    }

    fn poll(
        &mut self,
        ctx: &Services,
        _route: &AccountRoute,
        _modals: &mut ModalHost<Services, Self, AccountRoute>,
    ) -> Option<NavAction<AccountRoute>> {
        self.sync(ctx);
        None
    }

    fn keybindings(&self, _route: &AccountRoute) -> Vec<KeyHint> {
        self.keys.hints().to_vec()
    }

    fn render(&mut self, ctx: &Services, _route: &AccountRoute, frame: &mut Frame, area: Rect) {
        let theme = ctx.ui().theme();
        let [list_area, footer_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(area);
        let snapshot = ctx.accounts().state().snapshot();
        render_snapshot(
            ctx,
            &snapshot.view(),
            t!("accounts.loading").as_ref(),
            frame,
            list_area,
            |state, frame, area| self.render_body(ctx, state, frame, area),
        );
        let footer = match &self.error {
            Some(error) => Line::styled(error.clone(), Style::new().fg(theme.danger)),
            None => Line::default(),
        };
        frame.render_widget(Paragraph::new(footer), footer_area);
    }
}

#[derive(Default)]
pub(super) struct MicrosoftAuthPage;

impl Page<Services, AccountRoute> for MicrosoftAuthPage {
    fn on_key(
        &mut self,
        ctx: &Services,
        _route: &AccountRoute,
        key: KeyEvent,
    ) -> PageEvent<AccountRoute> {
        if key.code == KeyCode::Esc {
            ctx.accounts().cancel_authorization();
            PageEvent::Navigate(NavAction::Pop)
        } else {
            PageEvent::Ignored
        }
    }

    fn keybindings(&self, _route: &AccountRoute) -> Vec<KeyHint> {
        vec![KeyHint::new("ESC", t!("common.cancel"))]
    }

    fn poll(&mut self, ctx: &Services, _route: &AccountRoute) -> Option<NavAction<AccountRoute>> {
        if ctx.accounts().authorization() == AuthorizationState::Idle {
            Some(NavAction::Pop)
        } else {
            None
        }
    }

    fn render(&mut self, ctx: &Services, _route: &AccountRoute, frame: &mut Frame, area: Rect) {
        let theme = ctx.ui().theme();
        match ctx.accounts().authorization() {
            AuthorizationState::Idle => {}
            AuthorizationState::Starting => {
                render_loading(ctx, frame, area, t!("accounts.microsoft_starting").as_ref());
            }
            AuthorizationState::Awaiting {
                verification_uri,
                user_code,
            } => {
                let dim = Style::new().fg(theme.text_dim);
                let strong = Style::new().fg(theme.accent).add_modifier(Modifier::BOLD);
                let lines = vec![
                    Line::styled(
                        t!("accounts.microsoft_title"),
                        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
                    ),
                    Line::raw(""),
                    Line::from(vec![
                        Span::styled(t!("accounts.microsoft_open"), dim),
                        Span::styled(verification_uri, strong),
                    ]),
                    Line::from(vec![
                        Span::styled(t!("accounts.microsoft_code"), dim),
                        Span::styled(user_code, strong),
                    ]),
                    Line::raw(""),
                    Line::styled(t!("accounts.microsoft_waiting"), dim),
                ];
                frame.render_widget(Paragraph::new(lines), area);
            }
        }
    }
}
