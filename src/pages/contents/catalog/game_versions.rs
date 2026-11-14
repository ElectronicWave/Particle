use crate::t;
use elemental::{
    core::minecraft::{MinecraftVersion, MinecraftVersionId},
    driver::{catalog::GameVersion, driver::DriverDescriptor},
    launcher::DriverCatalog,
};
use ratail::SnapshotStatus;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    widgets::{List, ListItem, ListState},
};

use super::{ScreenAction, filter::VersionFilter};
use crate::app::services::Services;
use crate::pages::content::{render_loading, render_message};
use crate::theme::Theme;

#[derive(Debug)]
pub(super) struct GameVersionsScreen {
    versions: ListState,
}

impl Default for GameVersionsScreen {
    fn default() -> Self {
        Self {
            versions: ListState::default().with_selected(Some(0)),
        }
    }
}

impl GameVersionsScreen {
    pub(super) fn reset(&mut self) {
        self.versions = ListState::default().with_selected(Some(0));
    }

    pub(super) fn render(
        &mut self,
        ctx: &Services,
        frame: &mut Frame,
        area: Rect,
        driver: DriverDescriptor,
        filter: VersionFilter,
    ) {
        let theme = ctx.theme();
        let snapshot = ctx.catalog(driver).snapshot();
        let view = snapshot.view();
        match (view.data, view.error) {
            (Some(data), Some(error)) => {
                let [error_area, list_area] =
                    Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);
                render_message(frame, error_area, &t!("common.error", error = error));
                self.render_versions(data, frame, list_area, filter, theme);
            }
            (Some(data), None) => self.render_versions(data, frame, area, filter, theme),
            (None, Some(error)) => render_message(frame, area, &t!("common.error", error = error)),
            (None, None) if view.status == SnapshotStatus::Cancelled => {
                render_message(frame, area, &t!("common.cancelled"))
            }
            (None, None) => render_loading(ctx, frame, area, &t!("common.loading")),
        }
    }

    pub(super) fn on_key(
        &mut self,
        ctx: &Services,
        key: KeyEvent,
        driver: DriverDescriptor,
        filter: VersionFilter,
    ) -> ScreenAction {
        match key.code {
            KeyCode::Down => {
                self.versions.select_next();
                ScreenAction::Handled
            }
            KeyCode::Up => {
                self.versions.select_previous();
                ScreenAction::Handled
            }
            KeyCode::Enter => match self.selected_game(ctx, driver, filter) {
                Some(game) => ScreenAction::OpenLoader(game),
                None => ScreenAction::Handled,
            },
            _ => ScreenAction::Ignored,
        }
    }

    fn render_versions(
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

        frame.render_stateful_widget(list, area, &mut self.versions);
    }

    fn selected_game(
        &self,
        ctx: &Services,
        driver: DriverDescriptor,
        filter: VersionFilter,
    ) -> Option<MinecraftVersionId> {
        let snapshot = ctx.catalog(driver).snapshot();
        let data = snapshot.view().data?;
        let versions = data
            .iter()
            .filter(|version| filter.matches(version))
            .collect::<Vec<&GameVersion>>();
        let selected = self.versions.selected()?;

        versions.get(selected).map(|version| version.id.clone())
    }
}

pub async fn load_versions(driver: DriverDescriptor) -> Result<Vec<GameVersion>, anyhow::Error> {
    let catalog = DriverCatalog::for_driver(driver)?;
    let mut versions = catalog.game_versions().await?;

    // Newest first; unparseable ids sort last by raw id.
    versions.sort_by(|left, right| {
        let left_key = MinecraftVersion::parse_id(&left.id)
            .ok()
            .map(|version| version.key());
        let right_key = MinecraftVersion::parse_id(&right.id)
            .ok()
            .map(|version| version.key());
        right_key
            .cmp(&left_key)
            .then_with(|| right.id.as_str().cmp(left.id.as_str()))
    });
    Ok(versions)
}
