use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::Arc,
};

use elemental::core::auth::{
    accounts::AccountId, authorizers::offline::OFFLINE_AUTHORIZER_ID, texture::SkinModel,
};
use ratail::{
    InputEvent, KeyChord, KeyHint, KeyMap, Modal, ModalHost, ModalOutcome, ModalPage, NavAction,
    Navigator, Page, PageEvent, Router, Snapshot, SnapshotView,
};
use ratatui::{
    Frame,
    crossterm::event::KeyCode,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::pages::content::render_message;
use crate::pages::route::Route;
use crate::services::{CapeState, Services, SkinState};
use crate::t;
use crate::theme::Theme;
use crate::widgets::{
    filepicker::{FilePicker, FilePickerMode, FilePickerOptions},
    png::PngImage,
    text_edit::{EditEvent, TextEdit},
};

mod capes;
mod list;
mod skins;
mod yggdrasil;

use capes::CapesPage;
use list::{AccountsListPage, MicrosoftAuthPage};
use skins::SkinsPage;
use yggdrasil::YggdrasilPage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthorizerKind {
    Microsoft,
    Yggdrasil,
    LocalYggdrasil,
    Offline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AccountRoute {
    List,
    Skins(AccountId),
    Capes(AccountId),
    AddYggdrasil,
    EditYggdrasil {
        account_id: AccountId,
        base_url: String,
    },
    MicrosoftAuth,
}

struct AccountRouter;

impl Router<Services, AccountRoute> for AccountRouter {
    fn create(
        &mut self,
        route: &AccountRoute,
        _ctx: &Services,
    ) -> Box<dyn Page<Services, AccountRoute>> {
        match route {
            AccountRoute::List => Box::new(ModalPage::new(AccountsListPage::default())),
            AccountRoute::Skins(account_id) => {
                Box::new(ModalPage::new(SkinsPage::new(*account_id)))
            }
            AccountRoute::Capes(account_id) => {
                Box::new(ModalPage::new(CapesPage::new(*account_id)))
            }
            AccountRoute::AddYggdrasil => Box::new(YggdrasilPage::add()),
            AccountRoute::EditYggdrasil {
                account_id,
                base_url,
            } => Box::new(YggdrasilPage::edit(*account_id, base_url.clone())),
            AccountRoute::MicrosoftAuth => Box::new(MicrosoftAuthPage),
        }
    }
}

pub struct AccountsContent {
    nav: Navigator<Services, AccountRoute, AccountRouter>,
}

impl AccountsContent {
    pub fn new(ctx: &Services) -> Self {
        Self {
            nav: Navigator::new(AccountRouter, AccountRoute::List, ctx),
        }
    }
}

impl Page<Services> for AccountsContent {
    fn resume(&mut self, ctx: &Services, _route: &Route) {
        self.nav.resume_current(ctx);
    }

    fn pause(&mut self, ctx: &Services, _route: &Route) {
        self.nav.pause_current(ctx);
    }

    fn dispose(&mut self, ctx: &Services, _route: &Route) {
        self.nav.dispose_all(ctx);
    }

    fn render(&mut self, ctx: &Services, _route: &Route, frame: &mut Frame, area: Rect) {
        self.nav.render(ctx, frame, area);
    }

    fn on_input(&mut self, ctx: &Services, _route: &Route, input: InputEvent) -> PageEvent<Route> {
        if self.nav.handle_input(ctx, input) {
            PageEvent::Handled
        } else {
            PageEvent::Ignored
        }
    }

    fn poll(&mut self, ctx: &Services, _route: &Route) -> Option<NavAction<Route>> {
        self.nav.poll_current(ctx);
        None
    }

    fn keybindings(&self, _route: &Route) -> Vec<KeyHint> {
        self.nav.keybindings()
    }
}

struct NamePrompt {
    label: Cow<'static, str>,
    help: Cow<'static, str>,
    empty_error: Cow<'static, str>,
    edit: TextEdit,
    error: Option<String>,
}

impl NamePrompt {
    fn new(
        label: Cow<'static, str>,
        value: String,
        help: Cow<'static, str>,
        empty_error: Cow<'static, str>,
    ) -> Self {
        Self {
            label,
            help,
            empty_error,
            edit: TextEdit::new(value),
            error: None,
        }
    }
}

impl Modal<Services> for NamePrompt {
    type Value = String;

    fn on_input(&mut self, _ctx: &Services, input: InputEvent) -> ModalOutcome<String> {
        match self.edit.handle_input(&input) {
            EditEvent::Submit => {
                let name = self.edit.input().trim().to_owned();
                if name.is_empty() {
                    self.error = Some(self.empty_error.to_string());
                    ModalOutcome::Handled
                } else {
                    ModalOutcome::Completed(name)
                }
            }
            EditEvent::Changed => {
                self.error = None;
                ModalOutcome::Handled
            }
            EditEvent::Cancel => ModalOutcome::Cancelled,
            EditEvent::Unhandled => ModalOutcome::Ignored,
        }
    }

    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect) {
        let theme = ctx.ui().theme();
        let [_, footer] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(area);
        let help_style = match &self.error {
            Some(_) => Style::new().fg(theme.danger),
            None => Style::new().fg(theme.text_dim),
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(format!("{}: ", self.label), Style::new().fg(theme.accent)),
                    Span::raw(self.edit.cursor_display()),
                ]),
                Line::styled(
                    self.error.clone().unwrap_or_else(|| self.help.to_string()),
                    help_style,
                ),
            ]),
            footer,
        );
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        vec![
            KeyHint::new("ENTER", t!("accounts.confirm")),
            KeyHint::new("ESC", t!("common.cancel")),
        ]
    }
}

fn bind_list_nav<A: Copy>(
    keys: &mut KeyMap<A>,
    navigate: fn(KeyCode) -> A,
    choose: A,
    choose_desc: Cow<'static, str>,
) {
    keys.bind_with_hint(
        KeyChord::plain(KeyCode::Down),
        navigate(KeyCode::Down),
        KeyHint::new("↑/↓", t!("common.browse")),
    );
    for code in [KeyCode::Up, KeyCode::Home, KeyCode::End] {
        keys.bind(KeyChord::plain(code), navigate(code));
    }
    keys.bind_with_hint(
        KeyChord::plain(KeyCode::Enter),
        choose,
        KeyHint::new("ENTER", choose_desc),
    );
}

fn bind_remove_and_back<A: Copy>(keys: &mut KeyMap<A>, remove: Option<A>, back: A) {
    if let Some(remove) = remove {
        keys.bind_with_hint(
            KeyChord::char('d'),
            remove,
            KeyHint::new("D", t!("common.remove")),
        );
        keys.bind(KeyChord::plain(KeyCode::Delete), remove);
    }
    keys.bind_with_hint(
        KeyChord::plain(KeyCode::Esc),
        back,
        KeyHint::new("ESC", t!("common.return")),
    );
}

fn open_texture_import<P: 'static, F>(
    modals: &mut ModalHost<Services, P, AccountRoute>,
    picker_title: Cow<'static, str>,
    name_label: Cow<'static, str>,
    name_help: Cow<'static, str>,
    empty_error: Cow<'static, str>,
    on_submit: F,
) where
    F: FnOnce(&mut P, &Services, PathBuf, String) + 'static,
{
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    modals.open(
        FilePicker::open(
            FilePickerOptions {
                cwd,
                preferred_extensions: vec!["png".to_owned()],
            },
            FilePickerMode::File,
            picker_title,
        ),
        move |_page, _ctx, modals, path| {
            let name = default_texture_name(&path);
            modals.open(
                NamePrompt::new(name_label, name, name_help, empty_error),
                move |page, ctx, _modals, name| on_submit(page, ctx, path, name),
            );
        },
    );
}

fn default_texture_name(path: &Path) -> String {
    let Some(stem) = path.file_stem() else {
        return String::new();
    };
    let stem = stem.to_string_lossy();
    let compact = stem.chars().filter(|c| *c != '-').collect::<String>();
    if matches!(compact.len(), 32 | 40 | 64) && compact.chars().all(|c| c.is_ascii_hexdigit()) {
        String::new()
    } else {
        stem.into_owned()
    }
}

fn account_uses_local_textures(ctx: &Services, account_id: AccountId) -> bool {
    let snapshot = ctx.accounts().state().snapshot();
    snapshot.view().data.is_some_and(|state| {
        state
            .accounts
            .iter()
            .find(|account| account.id == account_id)
            .is_some_and(|account| account.authorizer == OFFLINE_AUTHORIZER_ID || account.local)
    })
}

fn texture_body_layout(area: Rect, empty: bool) -> (Rect, Rect) {
    if empty || area.width < 46 {
        return (area, Rect::new(area.x, area.y, 0, 0));
    }
    let width = (area.width / 3).clamp(18, 36);
    let [list, preview] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(width)]).areas(area);
    (list, preview)
}

fn render_preview_notice(frame: &mut Frame, area: Rect, message: &str, theme: Theme) {
    frame.render_widget(
        Paragraph::new(message)
            .centered()
            .style(Style::new().fg(theme.text_dim)),
        area,
    );
}

fn render_image_preview(
    snapshot: Option<Arc<Snapshot<PngImage, anyhow::Error>>>,
    frame: &mut Frame,
    area: Rect,
    theme: Theme,
) {
    let Some(snapshot) = snapshot else {
        render_preview_notice(frame, area, t!("common.loading").as_ref(), theme);
        return;
    };
    let view = snapshot.view();
    match (view.data, view.error) {
        (Some(image), _) => frame.render_widget(image.as_ref(), area),
        (None, Some(error)) => render_preview_notice(
            frame,
            area,
            t!("common.error", error = error).as_ref(),
            theme,
        ),
        (None, None) => render_preview_notice(frame, area, t!("common.loading").as_ref(), theme),
    }
}

fn render_list_message(frame: &mut Frame, area: Rect, message: &str) {
    let [_, message_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(area);
    render_message(frame, message_area, message);
}

fn render_texture_status(frame: &mut Frame, area: Rect, lines: (String, String), theme: Theme) {
    let dim = Style::new().fg(theme.text_dim);
    frame.render_widget(
        Paragraph::new(vec![Line::styled(lines.0, dim), Line::styled(lines.1, dim)]),
        area,
    );
}

trait AccountScoped {
    fn account_id(&self) -> AccountId;
}

impl AccountScoped for SkinState {
    fn account_id(&self) -> AccountId {
        self.account_id
    }
}

impl AccountScoped for CapeState {
    fn account_id(&self) -> AccountId {
        self.account_id
    }
}

fn for_account<'a, T: AccountScoped>(
    view: SnapshotView<'a, T, anyhow::Error>,
    account_id: AccountId,
) -> SnapshotView<'a, T, anyhow::Error> {
    SnapshotView {
        status: view.status,
        data: view.data.filter(|state| state.account_id() == account_id),
        error: view.error,
    }
}

fn skin_model_label(model: SkinModel) -> Cow<'static, str> {
    match model {
        SkinModel::Classic => t!("accounts.skin_classic"),
        SkinModel::Slim => t!("accounts.skin_slim"),
    }
}
