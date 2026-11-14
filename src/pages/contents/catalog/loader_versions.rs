use crate::t;
use elemental::{
    core::minecraft::MinecraftVersionId,
    driver::{catalog::LoaderRelease, driver::DriverDescriptor, loader_version::LoaderVersionId},
    launcher::{DriverCatalog, error::Error as LauncherError},
};
use ratail::SnapshotStatus;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};

use super::ScreenAction;
use crate::app::services::Services;
use crate::pages::content::{render_loading, render_message};
use crate::theme::Theme;

#[derive(Clone, Debug)]
pub(super) struct LoaderSelection {
    pub(super) game: MinecraftVersionId,
    pub(super) loader: Option<LoaderVersionId>,
}

#[derive(Debug)]
pub(super) struct LoaderVersionsScreen {
    game: MinecraftVersionId,
    loaders: ListState,
}

impl LoaderVersionsScreen {
    pub(super) fn new(game: MinecraftVersionId) -> Self {
        Self {
            game,
            loaders: ListState::default().with_selected(Some(0)),
        }
    }

    pub(super) fn game(&self) -> &MinecraftVersionId {
        &self.game
    }

    pub(super) fn render(
        &mut self,
        ctx: &Services,
        frame: &mut Frame,
        area: Rect,
        driver: DriverDescriptor,
        selection: Option<&LoaderSelection>,
    ) {
        let theme = ctx.theme();
        let snapshot = ctx.loader_versions(driver, &self.game).snapshot();
        let view = snapshot.view();
        match (view.data, view.error) {
            (Some(data), Some(error)) => {
                let [error_area, list_area] =
                    Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);
                render_message(frame, error_area, &t!("common.error", error = error));
                self.render_list(data, frame, list_area, selection, theme);
            }
            (Some(data), None) => self.render_list(data, frame, area, selection, theme),
            (None, Some(error)) => render_message(frame, area, &loader_error_message(error)),
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
    ) -> ScreenAction {
        match key.code {
            KeyCode::Esc => ScreenAction::Back,
            KeyCode::Down => {
                self.loaders.select_next();
                ScreenAction::Handled
            }
            KeyCode::Up => {
                self.loaders.select_previous();
                ScreenAction::Handled
            }
            KeyCode::Enter => match self.selected_loader(ctx, driver) {
                Some(loader) => ScreenAction::Selected(LoaderSelection {
                    game: self.game.clone(),
                    loader: Some(loader),
                }),
                None => ScreenAction::Handled,
            },
            _ => ScreenAction::Ignored,
        }
    }

    fn render_list(
        &mut self,
        data: &[LoaderRelease],
        frame: &mut Frame,
        area: Rect,
        selection: Option<&LoaderSelection>,
        theme: Theme,
    ) {
        if data.is_empty() {
            render_message(frame, area, &t!("catalog.no_loader_versions"));
            return;
        }

        let items = data
            .iter()
            .map(|release| {
                let selected = selection.is_some_and(|selection| {
                    selection.game == self.game
                        && selection.loader.as_ref() == Some(&release.loader_version)
                });
                let marker = if selected { "  ✓" } else { "" };
                let label = format!("{}{marker}", release.loader_version);
                ListItem::new(label)
            })
            .collect::<Vec<ListItem>>();
        let list = List::new(items).highlight_style(theme.selection_style());

        frame.render_stateful_widget(list, area, &mut self.loaders);
    }

    fn selected_loader(&self, ctx: &Services, driver: DriverDescriptor) -> Option<LoaderVersionId> {
        let snapshot = ctx.loader_versions(driver, &self.game).snapshot();
        let data = snapshot.view().data?;
        let selected = self.loaders.selected()?;

        data.get(selected)
            .map(|release| release.loader_version.clone())
    }
}

pub(super) fn render_header(
    frame: &mut Frame,
    area: Rect,
    game: &MinecraftVersionId,
    theme: Theme,
) {
    let line = Line::from(vec![
        Span::styled(game.to_string(), Style::new().add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("  ›  {}", t!("catalog.loader_versions")),
            Style::new().fg(theme.text_dim),
        ),
    ]);

    frame.render_widget(Paragraph::new(line), area);
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

pub async fn load_loader_versions(
    driver: DriverDescriptor,
    game: MinecraftVersionId,
) -> Result<Vec<LoaderRelease>, anyhow::Error> {
    let catalog = DriverCatalog::for_driver(driver)?;
    Ok(catalog.loader_versions(&game).await?)
}
