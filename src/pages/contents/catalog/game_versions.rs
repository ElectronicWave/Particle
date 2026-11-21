use elemental::{
    core::minecraft::MinecraftVersionId,
    driver::{catalog::GameVersion, descriptors::VANILLA_DRIVER},
};
use ratail::{KeyHint, NavAction, Page, PageEvent};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    widgets::{List, ListItem},
};

use super::VersionFilter;
use super::{CatalogRoute, LoaderSelection};
use crate::pages::content::{render_message, render_snapshot};
use crate::services::Services;
use crate::t;
use crate::theme::Theme;
use crate::widgets::list::ListSelection;

#[derive(Default)]
pub(super) struct GameVersionsPage {
    versions: ListSelection,
}

impl Page<Services, CatalogRoute> for GameVersionsPage {
    fn init(&mut self, ctx: &Services, route: &CatalogRoute) {
        let CatalogRoute::GameVersions { driver, .. } = route else {
            return;
        };
        let driver = ctx.catalog().drivers()[*driver];
        if ctx
            .catalog()
            .versions(driver)
            .snapshot()
            .view()
            .data
            .is_none()
        {
            ctx.catalog().load_versions(driver);
        }
    }

    fn on_key(
        &mut self,
        ctx: &Services,
        route: &CatalogRoute,
        key: KeyEvent,
    ) -> PageEvent<CatalogRoute> {
        let CatalogRoute::GameVersions { driver, filter } = route else {
            return PageEvent::Ignored;
        };
        match key.code {
            KeyCode::Down | KeyCode::Up | KeyCode::Home | KeyCode::End => {
                self.versions.handle_key(key.code);
                PageEvent::Handled
            }
            KeyCode::Enter => {
                let Some(game) = self.selected_game(ctx, *driver, *filter) else {
                    return PageEvent::Handled;
                };
                let driver_descriptor = ctx.catalog().drivers()[*driver];
                if driver_descriptor == VANILLA_DRIVER {
                    PageEvent::Navigate(NavAction::Push(CatalogRoute::InstanceName {
                        driver: *driver,
                        selection: LoaderSelection { game, loader: None },
                    }))
                } else {
                    ctx.catalog()
                        .load_loader_versions(driver_descriptor, game.clone());
                    PageEvent::Navigate(NavAction::Push(CatalogRoute::LoaderVersions {
                        driver: *driver,
                        game,
                        filter: *filter,
                    }))
                }
            }
            _ => PageEvent::Ignored,
        }
    }

    fn keybindings(&self, _route: &CatalogRoute) -> Vec<KeyHint> {
        super::catalog_hints()
    }

    fn render(&mut self, ctx: &Services, route: &CatalogRoute, frame: &mut Frame, area: Rect) {
        let CatalogRoute::GameVersions { driver, filter } = route else {
            return;
        };
        let theme = ctx.ui().theme();
        let [filter_area, body_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);
        super::render_filter_tabs(frame, filter_area, *filter, theme);
        let snapshot = ctx
            .catalog()
            .versions(ctx.catalog().drivers()[*driver])
            .snapshot();
        render_snapshot(
            ctx,
            &snapshot.view(),
            &t!("common.loading"),
            frame,
            body_area,
            |data, frame, area| self.render_list(data, frame, area, *filter, theme),
        );
    }
}

impl GameVersionsPage {
    fn render_list(
        &mut self,
        data: &[GameVersion],
        frame: &mut Frame,
        area: Rect,
        filter: VersionFilter,
        theme: Theme,
    ) {
        let versions = data
            .iter()
            .filter(|version| filter.matches(version))
            .collect::<Vec<&GameVersion>>();

        if versions.is_empty() {
            let message = match filter {
                VersionFilter::All => t!("catalog.no_versions"),
                _ => t!("catalog.no_channel_versions"),
            };
            render_message(frame, area, &message);
            return;
        }

        let items = versions
            .iter()
            .map(|version| ListItem::new(version.id.to_string()))
            .collect::<Vec<ListItem>>();
        let list = List::new(items).highlight_style(theme.selection_style());

        frame.render_stateful_widget(list, area, self.versions.state_mut());
    }

    fn selected_game(
        &self,
        ctx: &Services,
        driver: usize,
        filter: VersionFilter,
    ) -> Option<MinecraftVersionId> {
        let snapshot = ctx
            .catalog()
            .versions(ctx.catalog().drivers()[driver])
            .snapshot();
        let data = snapshot.view().data?;
        let versions = data
            .iter()
            .filter(|version| filter.matches(version))
            .collect::<Vec<&GameVersion>>();
        let selected = self.versions.selected()?;

        versions.get(selected).map(|version| version.id.clone())
    }
}
