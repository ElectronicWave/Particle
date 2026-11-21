use std::borrow::Cow;

use elemental::{
    core::auth::{accounts::AccountId, authorizers::offline::OFFLINE_AUTHORIZER_ID},
    launcher::CapeId,
};
use ratail::{KeyChord, KeyHint, KeyMap, ModalHost, NavAction, PageEvent, WithModal};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    widgets::List,
};

use crate::pages::content::render_snapshot;
use crate::services::{CapeAccountKind, CapeState, OnlineCape, Services};
use crate::t;
use crate::theme::Theme;
use crate::widgets::{confirm::Confirmation, list::ListSelection};

use super::{
    AccountRoute, NamePrompt, bind_list_nav, bind_remove_and_back, for_account,
    open_texture_import, render_image_preview, render_list_message, render_texture_status,
    texture_body_layout,
};

#[derive(Debug, Clone, Copy)]
enum CapeAction {
    Navigate(KeyCode),
    Choose,
    Import,
    Clear,
    Rename,
    Remove,
    Back,
}

pub(super) struct CapesPage {
    account_id: AccountId,
    list: ListSelection,
    kind: CapeAccountKind,
    keys: KeyMap<CapeAction>,
}

impl CapesPage {
    pub(super) fn new(account_id: AccountId) -> Self {
        Self {
            account_id,
            list: ListSelection::wrap(0),
            kind: CapeAccountKind::Local,
            keys: cape_keys(CapeAccountKind::Local),
        }
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
            CapeAction::Navigate(code) => {
                self.list.handle_key(code);
            }
            CapeAction::Choose => self.choose(ctx),
            CapeAction::Import => open_import(modals),
            CapeAction::Clear => {
                ctx.capes().clear(self.account_id);
            }
            CapeAction::Rename => self.rename(ctx, modals),
            CapeAction::Remove => self.remove(ctx, modals),
            CapeAction::Back => return PageEvent::Navigate(NavAction::Pop),
        }

        PageEvent::Handled
    }

    fn choose(&self, ctx: &Services) {
        if self.kind == CapeAccountKind::Microsoft {
            if let Some(cape_id) = self.selected_online(ctx).and_then(|cape| cape.id) {
                ctx.capes().select_online(self.account_id, Some(cape_id));
            }
        } else if let Some((cape_id, _)) = self.selected_entry(ctx) {
            ctx.capes().choose(self.account_id, cape_id);
        }
    }

    fn rename(&self, ctx: &Services, modals: &mut ModalHost<Services, Self, AccountRoute>) {
        let Some((cape_id, name)) = self.selected_entry(ctx) else {
            return;
        };
        let account_id = self.account_id;
        modals.open(
            NamePrompt::new(
                t!("accounts.cape_name"),
                name,
                t!("accounts.rename_help"),
                t!("accounts.name_required"),
            ),
            move |_page, ctx, _modals, name| {
                ctx.capes().rename(account_id, cape_id, name);
            },
        );
    }

    fn remove(&self, ctx: &Services, modals: &mut ModalHost<Services, Self, AccountRoute>) {
        let Some((cape_id, name)) = self.selected_entry(ctx) else {
            return;
        };
        let title = t!("accounts.confirm_remove_cape", cape = name).into_owned();
        modals.open(
            Confirmation::open(title, cape_id),
            |page, ctx, _modals, id| {
                ctx.capes().remove(page.account_id, id);
            },
        );
    }

    fn sync_preview(&mut self, ctx: &Services) {
        let snapshot = ctx.capes().snapshot();
        let Some(state) = for_account(snapshot.view(), self.account_id).data else {
            return;
        };
        self.set_kind(state.kind);
        let index = self.list.index();
        match state.kind {
            CapeAccountKind::Microsoft => {
                if let Some(cape) = state.selectable_online().nth(index) {
                    ctx.capes().preview_online(&cape.url);
                }
            }
            CapeAccountKind::Local | CapeAccountKind::Online => {
                if let Some(entry) = state.entries.get(index) {
                    ctx.capes().preview_local(&entry.path);
                }
            }
        }
    }

    fn selected_entry(&self, ctx: &Services) -> Option<(CapeId, String)> {
        let snapshot = ctx.capes().snapshot();
        let state = for_account(snapshot.view(), self.account_id).data?;
        state
            .entries
            .get(self.list.selected()?)
            .map(|entry| (entry.asset.id, entry.asset.name.clone()))
    }

    fn selected_online(&self, ctx: &Services) -> Option<OnlineCape> {
        let snapshot = ctx.capes().snapshot();
        let state = for_account(snapshot.view(), self.account_id).data?;
        state
            .selectable_online()
            .nth(self.list.selected()?)
            .cloned()
    }

    fn set_kind(&mut self, kind: CapeAccountKind) {
        if self.kind == kind {
            return;
        }
        self.kind = kind;
        self.keys = cape_keys(kind);
    }

    fn render_list(&mut self, state: &CapeState, frame: &mut Frame, area: Rect, theme: Theme) {
        if state.kind == CapeAccountKind::Microsoft {
            let capes = state.selectable_online().collect::<Vec<_>>();
            self.list.set_len(capes.len());
            if capes.is_empty() {
                render_list_message(frame, area, t!("accounts.capes_empty_online").as_ref());
                return;
            }
            let items = capes
                .iter()
                .map(|cape| {
                    let marker = if cape.active { "*" } else { " " };
                    Line::styled(format!("{marker} {}", online_cape_name(cape)), Style::new())
                })
                .collect::<Vec<Line>>();
            let list = List::new(items).highlight_style(theme.selection_style());
            frame.render_stateful_widget(list, area, self.list.state_mut());
            return;
        }

        self.list.set_len(state.entries.len());
        if state.entries.is_empty() {
            render_list_message(frame, area, t!("accounts.capes_empty").as_ref());
            return;
        }
        let items = state
            .entries
            .iter()
            .map(|entry| {
                let selected = state
                    .selected
                    .as_ref()
                    .is_some_and(|selected| selected.asset.id == entry.asset.id);
                let marker = if selected { "*" } else { " " };
                Line::styled(format!("{marker} {}", entry.asset.name), Style::new())
            })
            .collect::<Vec<Line>>();
        let list = List::new(items).highlight_style(theme.selection_style());
        frame.render_stateful_widget(list, area, self.list.state_mut());
    }

    fn preview_snapshot(
        &self,
        ctx: &Services,
        state: &CapeState,
    ) -> Option<std::sync::Arc<ratail::Snapshot<crate::widgets::png::PngImage, anyhow::Error>>>
    {
        let selected = self.list.selected()?;
        match state.kind {
            CapeAccountKind::Microsoft => ctx
                .capes()
                .preview_online_snapshot(&state.selectable_online().nth(selected)?.url),
            CapeAccountKind::Local | CapeAccountKind::Online => ctx
                .capes()
                .preview_local_snapshot(&state.entries.get(selected)?.path),
        }
    }
}

impl WithModal<Services, AccountRoute> for CapesPage {
    fn init(
        &mut self,
        ctx: &Services,
        _route: &AccountRoute,
        _modals: &mut ModalHost<Services, Self, AccountRoute>,
    ) {
        if let Some(account) = ctx
            .accounts()
            .state()
            .snapshot()
            .view()
            .data
            .and_then(|state| {
                state
                    .accounts
                    .iter()
                    .find(|account| account.id == self.account_id)
            })
        {
            self.set_kind(CapeAccountKind::from_account(
                &account.authorizer,
                account.local || account.authorizer == OFFLINE_AUTHORIZER_ID,
            ));
        }
        ctx.capes().load(self.account_id);
    }

    fn resume(
        &mut self,
        ctx: &Services,
        _route: &AccountRoute,
        _modals: &mut ModalHost<Services, Self, AccountRoute>,
    ) {
        self.sync_preview(ctx);
    }

    fn on_key(
        &mut self,
        ctx: &Services,
        _route: &AccountRoute,
        modals: &mut ModalHost<Services, Self, AccountRoute>,
        key: KeyEvent,
    ) -> PageEvent<AccountRoute> {
        let event = self.handle_key(ctx, modals, key);
        if event == PageEvent::Handled {
            self.sync_preview(ctx);
        }
        event
    }

    fn poll(
        &mut self,
        ctx: &Services,
        _route: &AccountRoute,
        _modals: &mut ModalHost<Services, Self, AccountRoute>,
    ) -> Option<NavAction<AccountRoute>> {
        self.sync_preview(ctx);
        None
    }

    fn keybindings(&self, _route: &AccountRoute) -> Vec<KeyHint> {
        self.keys.hints().to_vec()
    }

    fn render(&mut self, ctx: &Services, _route: &AccountRoute, frame: &mut Frame, area: Rect) {
        let theme = ctx.ui().theme();
        let [body_area, status_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(area);
        let snapshot = ctx.capes().snapshot();
        let view = for_account(snapshot.view(), self.account_id);
        let empty = view.data.is_some_and(|state| match state.kind {
            CapeAccountKind::Microsoft => state.selectable_online().next().is_none(),
            CapeAccountKind::Local | CapeAccountKind::Online => state.entries.is_empty(),
        });
        let (list_area, preview_area) = texture_body_layout(body_area, empty);
        render_snapshot(
            ctx,
            &view,
            t!("accounts.capes_loading").as_ref(),
            frame,
            list_area,
            |state, frame, area| self.render_list(state, frame, area, theme),
        );
        if preview_area.width > 0
            && let Some(state) = view.data
        {
            render_image_preview(
                self.preview_snapshot(ctx, state),
                frame,
                preview_area,
                theme,
            );
        }
        render_texture_status(
            frame,
            status_area,
            cape_status(view.data.map(AsRef::as_ref), self.kind),
            theme,
        );
    }
}

fn online_cape_name(cape: &OnlineCape) -> String {
    cape.alias
        .as_deref()
        .filter(|alias| !alias.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| t!("accounts.cape_online_unnamed").into_owned())
}

fn cape_status(state: Option<&CapeState>, kind: CapeAccountKind) -> (String, String) {
    if kind == CapeAccountKind::Microsoft {
        let name = state
            .and_then(|state| state.online.iter().find(|cape| cape.active))
            .map(online_cape_name)
            .unwrap_or_else(|| t!("accounts.cape_none").into_owned());
        return (
            t!("accounts.cape_selected", cape = name).into_owned(),
            t!("accounts.cape_online_help").into_owned(),
        );
    }

    let cape = state
        .and_then(|state| state.selected.as_ref())
        .map(|selected| Cow::Borrowed(selected.asset.name.as_str()))
        .unwrap_or_else(|| t!("accounts.cape_none"));
    let help = if kind == CapeAccountKind::Online {
        t!("accounts.cape_upload_help").into_owned()
    } else {
        t!("accounts.cape_scope_help").into_owned()
    };
    (t!("accounts.cape_selected", cape = cape).into_owned(), help)
}

fn cape_keys(kind: CapeAccountKind) -> KeyMap<CapeAction> {
    let mut keys = KeyMap::new();
    bind_list_nav(
        &mut keys,
        CapeAction::Navigate,
        CapeAction::Choose,
        t!("accounts.cape_select"),
    );
    if kind == CapeAccountKind::Microsoft {
        bind_remove_and_back(&mut keys, None, CapeAction::Back);
        return keys;
    }
    keys.bind_with_hint(
        KeyChord::char('i'),
        CapeAction::Import,
        KeyHint::new("I", t!("accounts.cape_import")),
    );
    keys.bind_with_hint(
        KeyChord::char('r'),
        CapeAction::Rename,
        KeyHint::new("R", t!("accounts.rename")),
    );
    if kind == CapeAccountKind::Local {
        keys.bind_with_hint(
            KeyChord::char('c'),
            CapeAction::Clear,
            KeyHint::new("C", t!("accounts.cape_clear")),
        );
    }
    bind_remove_and_back(&mut keys, Some(CapeAction::Remove), CapeAction::Back);
    keys
}

fn open_import(modals: &mut ModalHost<Services, CapesPage, AccountRoute>) {
    open_texture_import(
        modals,
        t!("accounts.cape_pick"),
        t!("accounts.cape_name"),
        t!("accounts.cape_name_help"),
        t!("accounts.name_required"),
        |page, ctx, path, name| {
            ctx.capes().import(page.account_id, name, path);
        },
    );
}
