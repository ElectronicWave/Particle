use elemental::{
    core::auth::{accounts::AccountId, texture::SkinModel},
    launcher::SkinId,
};
use ratail::{KeyChord, KeyHint, KeyMap, ModalHost, NavAction, PageEvent, WithModal};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::List,
};

use crate::pages::content::render_snapshot;
use crate::services::{Services, SkinState};
use crate::t;
use crate::theme::Theme;
use crate::widgets::{confirm::Confirmation, list::ListSelection, skin::SkinView};

use super::{
    AccountRoute, NamePrompt, account_uses_local_textures, bind_list_nav, bind_remove_and_back,
    for_account, open_texture_import, render_image_preview, render_list_message,
    render_texture_status, skin_model_label, texture_body_layout,
};

#[derive(Debug, Clone, Copy)]
struct SkinSelection {
    id: SkinId,
    model: SkinModel,
}

#[derive(Debug, Clone, Copy)]
enum SkinAction {
    Navigate(KeyCode),
    Choose,
    Model,
    Import,
    Clear,
    Rename,
    Remove,
    Back,
}

pub(super) struct SkinsPage {
    account_id: AccountId,
    list: ListSelection,
    model: SkinModel,
    view: SkinView,
    local_selection: bool,
    keys: KeyMap<SkinAction>,
}

impl SkinsPage {
    pub(super) fn new(account_id: AccountId) -> Self {
        Self {
            account_id,
            list: ListSelection::wrap(0),
            model: SkinModel::Classic,
            view: SkinView::Full,
            local_selection: false,
            keys: skin_keys(false),
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
            SkinAction::Navigate(code) => {
                self.list.handle_key(code);
            }
            SkinAction::Choose => self.choose(ctx),
            SkinAction::Model => self.toggle_model(ctx),
            SkinAction::Import => open_import(modals),
            SkinAction::Clear => {
                ctx.skins().clear(self.account_id);
            }
            SkinAction::Rename => self.rename(ctx, modals),
            SkinAction::Remove => self.remove(ctx, modals),
            SkinAction::Back => return PageEvent::Navigate(NavAction::Pop),
        }

        PageEvent::Handled
    }

    fn choose(&self, ctx: &Services) {
        if let Some(selection) = self.selected(ctx) {
            ctx.skins()
                .choose(self.account_id, selection.id, selection.model);
        }
    }

    fn toggle_model(&mut self, ctx: &Services) {
        if let Some(selection) = self.selected(ctx) {
            let model = toggle_model(selection.model);
            ctx.skins().set_model(self.account_id, selection.id, model);
        } else {
            self.model = toggle_model(self.model);
        }
    }

    fn rename(&self, ctx: &Services, modals: &mut ModalHost<Services, Self, AccountRoute>) {
        let Some((skin_id, name)) = self.selected_entry(ctx) else {
            return;
        };
        let account_id = self.account_id;
        modals.open(
            NamePrompt::new(
                t!("accounts.skin_name"),
                name,
                t!("accounts.rename_help"),
                t!("accounts.name_required"),
            ),
            move |_page, ctx, _modals, name| {
                ctx.skins().rename(account_id, skin_id, name);
            },
        );
    }

    fn remove(&self, ctx: &Services, modals: &mut ModalHost<Services, Self, AccountRoute>) {
        let Some((skin_id, name)) = self.selected_entry(ctx) else {
            return;
        };
        let title = t!("accounts.confirm_remove_skin", skin = name).into_owned();
        modals.open(
            Confirmation::open(title, skin_id),
            |page, ctx, _modals, id| {
                ctx.skins().remove(page.account_id, id);
            },
        );
    }

    fn sync_preview(&mut self, ctx: &Services) {
        let snapshot = ctx.skins().snapshot();
        let Some(state) = snapshot
            .view()
            .data
            .filter(|state| state.account_id == self.account_id)
        else {
            return;
        };
        let Some(entry) = state.entries.get(self.list.index()) else {
            return;
        };
        ctx.skins()
            .preview_texture(&entry.path, entry.asset.default_model, self.view);
    }

    fn set_local_selection(&mut self, local_selection: bool) {
        if self.local_selection != local_selection {
            self.local_selection = local_selection;
            self.keys = skin_keys(local_selection);
        }
    }

    fn selected(&self, ctx: &Services) -> Option<SkinSelection> {
        let snapshot = ctx.skins().snapshot();
        let state = for_account(snapshot.view(), self.account_id).data?;
        let entry = state.entries.get(self.list.selected()?)?;
        Some(SkinSelection {
            id: entry.asset.id,
            model: entry.asset.default_model,
        })
    }

    fn selected_entry(&self, ctx: &Services) -> Option<(SkinId, String)> {
        let snapshot = ctx.skins().snapshot();
        let state = for_account(snapshot.view(), self.account_id).data?;
        state
            .entries
            .get(self.list.selected()?)
            .map(|entry| (entry.asset.id, entry.asset.name.clone()))
    }

    fn render_list(&mut self, state: &SkinState, frame: &mut Frame, area: Rect, theme: Theme) {
        self.list.set_len(state.entries.len());
        if state.entries.is_empty() {
            render_list_message(frame, area, t!("accounts.skins_empty").as_ref());
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
                Line::from(vec![
                    Span::styled(format!("{marker} {}", entry.asset.name), Style::new()),
                    Span::styled(
                        format!(
                            "  ({})",
                            skin_model_label(entry.asset.default_model).as_ref()
                        ),
                        Style::new().fg(theme.text_dim),
                    ),
                ])
            })
            .collect::<Vec<Line>>();
        let list = List::new(items).highlight_style(theme.selection_style());
        frame.render_stateful_widget(list, area, self.list.state_mut());
    }
}

impl WithModal<Services, AccountRoute> for SkinsPage {
    fn init(
        &mut self,
        ctx: &Services,
        _route: &AccountRoute,
        _modals: &mut ModalHost<Services, Self, AccountRoute>,
    ) {
        let snapshot = ctx.skins().snapshot();
        if let Some(selected) = snapshot
            .view()
            .data
            .filter(|state| state.account_id == self.account_id)
            .and_then(|state| state.selected.as_ref())
        {
            self.model = selected.model;
        }
        self.set_local_selection(account_uses_local_textures(ctx, self.account_id));
        ctx.skins().load(self.account_id);
    }

    fn resume(
        &mut self,
        ctx: &Services,
        _route: &AccountRoute,
        _modals: &mut ModalHost<Services, Self, AccountRoute>,
    ) {
        self.set_local_selection(account_uses_local_textures(ctx, self.account_id));
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
        let snapshot = ctx.skins().snapshot();
        let view = for_account(snapshot.view(), self.account_id);
        let selected_entry = view.data.and_then(|state| {
            self.list
                .selected()
                .and_then(|index| state.entries.get(index))
        });
        let current_model = selected_entry
            .map(|entry| entry.asset.default_model)
            .unwrap_or(self.model);
        let empty = view.data.is_some_and(|state| state.entries.is_empty());
        let (list_area, preview_area) = texture_body_layout(body_area, empty);
        render_snapshot(
            ctx,
            &view,
            t!("accounts.skins_loading").as_ref(),
            frame,
            list_area,
            |state, frame, area| self.render_list(state, frame, area, theme),
        );
        if preview_area.width > 0
            && let Some(entry) = selected_entry
        {
            render_image_preview(
                ctx.skins()
                    .preview_texture_snapshot(&entry.path, current_model, self.view),
                frame,
                preview_area,
                theme,
            );
        }
        render_texture_status(
            frame,
            status_area,
            skin_status(view.data.map(AsRef::as_ref), current_model),
            theme,
        );
    }
}

fn skin_status(state: Option<&SkinState>, current_model: SkinModel) -> (String, String) {
    let model = skin_model_label(current_model);
    let skin = state
        .and_then(|state| state.selected.as_ref())
        .map(|selected| {
            t!(
                "accounts.skin_selected_model",
                skin = selected.asset.name.as_str(),
                model = skin_model_label(selected.model).as_ref()
            )
            .into_owned()
        })
        .unwrap_or_else(|| {
            t!(
                "accounts.skin_selected",
                skin = t!("accounts.skin_none").as_ref()
            )
            .into_owned()
        });
    (
        t!("accounts.skin_current_model", model = model.as_ref()).into_owned(),
        skin,
    )
}

fn skin_keys(clearable: bool) -> KeyMap<SkinAction> {
    let mut keys = KeyMap::new();
    bind_list_nav(
        &mut keys,
        SkinAction::Navigate,
        SkinAction::Choose,
        t!("accounts.skin_select"),
    );
    keys.bind_with_hint(
        KeyChord::char('i'),
        SkinAction::Import,
        KeyHint::new("I", t!("accounts.skin_import")),
    );
    keys.bind_with_hint(
        KeyChord::char('m'),
        SkinAction::Model,
        KeyHint::new("M", t!("accounts.skin_model")),
    );
    keys.bind_with_hint(
        KeyChord::char('r'),
        SkinAction::Rename,
        KeyHint::new("R", t!("accounts.rename")),
    );
    if clearable {
        keys.bind_with_hint(
            KeyChord::char('c'),
            SkinAction::Clear,
            KeyHint::new("C", t!("accounts.skin_clear")),
        );
    }
    bind_remove_and_back(&mut keys, Some(SkinAction::Remove), SkinAction::Back);
    keys
}

fn open_import(modals: &mut ModalHost<Services, SkinsPage, AccountRoute>) {
    open_texture_import(
        modals,
        t!("accounts.skin_pick"),
        t!("accounts.skin_name"),
        t!("accounts.skin_name_help"),
        t!("accounts.skin_name_required"),
        |page, ctx, path, name| {
            let model = page
                .selected(ctx)
                .map(|selection| selection.model)
                .unwrap_or(page.model);
            ctx.skins().import(page.account_id, name, path, model);
        },
    );
}

fn toggle_model(model: SkinModel) -> SkinModel {
    match model {
        SkinModel::Classic => SkinModel::Slim,
        SkinModel::Slim => SkinModel::Classic,
    }
}
