use elemental::{
    core::minecraft::MinecraftVersionId,
    driver::{catalog::LoaderRelease, loader_version::LoaderVersionId},
    launcher::error::Error as LauncherError,
};
use ratail::{KeyHint, NavAction, Page, PageEvent, SnapshotStatus};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    widgets::{List, ListItem},
};

use super::{CatalogRoute, LoaderSelection};
use crate::pages::content::{render_loading, render_message};
use crate::services::Services;
use crate::t;
use crate::theme::Theme;
use crate::widgets::list::ListSelection;

#[derive(Default)]
pub(super) struct LoaderVersionsPage {
    loaders: ListSelection,
    selected: Option<LoaderVersionId>,
}

impl Page<Services, CatalogRoute> for LoaderVersionsPage {
    fn init(&mut self, ctx: &Services, route: &CatalogRoute) {
        let CatalogRoute::LoaderVersions { driver, game, .. } = route else {
            return;
        };
        let driver = ctx.catalog().drivers()[*driver];
        if ctx
            .catalog()
            .loader_versions(driver, game)
            .snapshot()
            .view()
            .data
            .is_none()
        {
            ctx.catalog().load_loader_versions(driver, game.clone());
        }
    }

    fn on_key(
        &mut self,
        ctx: &Services,
        route: &CatalogRoute,
        key: KeyEvent,
    ) -> PageEvent<CatalogRoute> {
        let CatalogRoute::LoaderVersions { driver, game, .. } = route else {
            return PageEvent::Ignored;
        };
        match key.code {
            KeyCode::Esc => PageEvent::Navigate(NavAction::Pop),
            KeyCode::Down | KeyCode::Up | KeyCode::Home | KeyCode::End => {
                self.loaders.handle_key(key.code);
                PageEvent::Handled
            }
            KeyCode::Enter => match self.selected_loader(ctx, *driver, game) {
                Some(loader) => {
                    self.selected = Some(loader.clone());
                    let selection = LoaderSelection {
                        game: game.clone(),
                        loader: Some(loader),
                    };
                    PageEvent::Navigate(NavAction::Push(CatalogRoute::InstanceName {
                        driver: *driver,
                        selection,
                    }))
                }
                None => PageEvent::Handled,
            },
            _ => PageEvent::Ignored,
        }
    }

    fn keybindings(&self, _route: &CatalogRoute) -> Vec<KeyHint> {
        super::loader_hints()
    }

    fn render(&mut self, ctx: &Services, route: &CatalogRoute, frame: &mut Frame, area: Rect) {
        let CatalogRoute::LoaderVersions { driver, game, .. } = route else {
            return;
        };
        let theme = ctx.ui().theme();
        let [header_area, body_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);
        super::render_flow_header(
            frame,
            header_area,
            game.as_ref(),
            &t!("catalog.loader_versions"),
            theme,
        );

        let snapshot = ctx
            .catalog()
            .loader_versions(ctx.catalog().drivers()[*driver], game)
            .snapshot();
        let view = snapshot.view();
        match (view.data, view.error) {
            (Some(data), Some(error)) => {
                let [error_area, list_area] =
                    Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(body_area);
                render_message(frame, error_area, &t!("common.error", error = error));
                self.render_list(data, frame, list_area, theme);
            }
            (Some(data), None) => self.render_list(data, frame, body_area, theme),
            (None, Some(error)) => render_message(frame, body_area, &loader_error_message(error)),
            (None, None) if view.status == SnapshotStatus::Cancelled => {
                render_message(frame, body_area, &t!("common.cancelled"))
            }
            (None, None) => render_loading(ctx, frame, body_area, &t!("common.loading")),
        }
    }
}

impl LoaderVersionsPage {
    fn render_list(&mut self, data: &[LoaderRelease], frame: &mut Frame, area: Rect, theme: Theme) {
        if data.is_empty() {
            render_message(frame, area, &t!("catalog.no_loader_versions"));
            return;
        }

        let items = data
            .iter()
            .map(|release| {
                let selected = self.selected.as_ref() == Some(&release.loader_version);
                let marker = if selected { "  ✓" } else { "" };
                ListItem::new(format!("{}{marker}", release.loader_version))
            })
            .collect::<Vec<ListItem>>();
        let list = List::new(items).highlight_style(theme.selection_style());

        frame.render_stateful_widget(list, area, self.loaders.state_mut());
    }

    fn selected_loader(
        &self,
        ctx: &Services,
        driver: usize,
        game: &MinecraftVersionId,
    ) -> Option<LoaderVersionId> {
        let snapshot = ctx
            .catalog()
            .loader_versions(ctx.catalog().drivers()[driver], game)
            .snapshot();
        let data = snapshot.view().data?;
        let selected = self.loaders.selected()?;

        data.get(selected)
            .map(|release| release.loader_version.clone())
    }
}

fn loader_error_message(error: &anyhow::Error) -> String {
    if matches!(
        error.downcast_ref::<LauncherError>(),
        Some(LauncherError::LoaderVersionsUnsupported { .. })
    ) {
        t!("catalog.unsupported_loader").into_owned()
    } else {
        t!("common.error", error = error).into_owned()
    }
}
