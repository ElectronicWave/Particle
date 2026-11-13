use std::collections::HashMap;

use elemental::{
    core::minecraft::MinecraftVersionId,
    driver::{
        catalog::Release,
        drivers::{
            cleanroom::catalog::CleanroomCatalog, fabric::catalog::FabricCatalog,
            forge::catalog::ForgeCatalog, liteloader::catalog::LiteLoaderCatalog,
            neoforge::catalog::NeoForgeCatalog, quilt::catalog::QuiltCatalog,
            rift::catalog::RiftCatalog, vanilla::catalog::VanillaCatalog,
        },
    },
    launcher::Launcher,
};
use ratail::{KeyHint, Snapshot};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Paragraph, StatefulWidget, Widget},
};

use crate::app::services::Services;
use crate::pages::content::{Content, ContentEvent, container};
use crate::widgets::selected::{SelectedScrollView, SelectedScrollViewState};

const CATALOG_HINTS: &[KeyHint] = &[
    KeyHint::from_static("←/→", "Driver"),
    KeyHint::from_static("↑/↓", "Scroll"),
    KeyHint::from_static("ESC", "Return"),
];

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
pub enum CatalogDriver {
    #[default]
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
    Cleanroom,
    LiteLoader,
    Rift,
}

const CATALOG_DRIVERS: [CatalogDriver; 8] = [
    CatalogDriver::Vanilla,
    CatalogDriver::Fabric,
    CatalogDriver::Quilt,
    CatalogDriver::Forge,
    CatalogDriver::NeoForge,
    CatalogDriver::Cleanroom,
    CatalogDriver::LiteLoader,
    CatalogDriver::Rift,
];

impl CatalogDriver {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Vanilla => "Vanilla",
            Self::Fabric => "Fabric",
            Self::Quilt => "Quilt",
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
            Self::Cleanroom => "Cleanroom",
            Self::LiteLoader => "LiteLoader",
            Self::Rift => "Rift",
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Vanilla => 0,
            Self::Fabric => 1,
            Self::Quilt => 2,
            Self::Forge => 3,
            Self::NeoForge => 4,
            Self::Cleanroom => 5,
            Self::LiteLoader => 6,
            Self::Rift => 7,
        }
    }

    fn next(self) -> Self {
        let next_index = self
            .index()
            .saturating_add(1)
            .min(CATALOG_DRIVERS.len() - 1);
        CATALOG_DRIVERS[next_index]
    }

    fn previous(self) -> Self {
        let previous_index = self.index().saturating_sub(1);
        CATALOG_DRIVERS[previous_index]
    }
}

#[derive(Debug, Default)]
pub struct CatalogContent {
    scroll: SelectedScrollViewState,
    driver: CatalogDriver,
}

impl Content for CatalogContent {
    fn init(&mut self, ctx: &Services) {
        ctx.load_catalog(self.driver);
    }

    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect) {
        let inner = container(frame, area);
        let [header_area, body_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(inner);
        render_header(frame, header_area, self.driver);

        let snapshot = ctx.catalog(self.driver).snapshot();

        match &*snapshot {
            Snapshot::Idle => render_message(frame, body_area, "Loading..."),
            Snapshot::Loading { previous } => match previous {
                Some(data) => self.render_versions(data, frame, body_area),
                None => render_message(frame, body_area, "Loading..."),
            },
            Snapshot::Ready(data) => self.render_versions(data, frame, body_area),
            Snapshot::Error { error, previous } => match previous {
                Some(data) => {
                    let [error_area, list_area] =
                        Layout::vertical([Constraint::Length(1), Constraint::Fill(1)])
                            .areas(body_area);
                    render_message(frame, error_area, &format!("Error: {error}"));
                    self.render_versions(data, frame, list_area);
                }
                None => render_message(frame, body_area, &format!("Error: {error}")),
            },
            Snapshot::Cancelled { previous } => match previous {
                Some(data) => self.render_versions(data, frame, body_area),
                None => render_message(frame, body_area, "Cancelled"),
            },
        }
    }

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> ContentEvent {
        match key.code {
            KeyCode::Left => {
                self.switch_driver(ctx, self.driver.previous());
                return ContentEvent::Handled;
            }
            KeyCode::Right => {
                self.switch_driver(ctx, self.driver.next());
                return ContentEvent::Handled;
            }
            _ => {}
        }

        let snapshot = ctx.catalog(self.driver).snapshot();
        let Some(data) = snapshot.value().or_else(|| snapshot.previous()) else {
            return ContentEvent::Ignored;
        };

        match key.code {
            KeyCode::Down => {
                self.scroll.down(data.len());
                ContentEvent::Handled
            }
            KeyCode::Up => {
                self.scroll.up();
                ContentEvent::Handled
            }
            _ => ContentEvent::Ignored,
        }
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        CATALOG_HINTS.to_vec()
    }
}

impl CatalogContent {
    fn switch_driver(&mut self, ctx: &Services, driver: CatalogDriver) {
        if driver == self.driver {
            return;
        }

        self.driver = driver;
        self.scroll = SelectedScrollViewState::default();
        ctx.load_catalog(driver);
    }

    fn render_versions(&mut self, data: &[VersionData], frame: &mut Frame, area: Rect) {
        if data.is_empty() {
            render_message(frame, area, "No versions found");
            return;
        }

        let widget = SelectedScrollView::new(data, |_, version, is_selected, area, buf| {
            let style = if is_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };

            let text = version.label();

            Paragraph::new(text).style(style).render(area, buf);
        });
        widget.render(area, frame.buffer_mut(), &mut self.scroll);
    }
}

fn render_header(frame: &mut Frame, area: Rect, driver: CatalogDriver) {
    let text = format!("Driver: {}", driver.label());
    Paragraph::new(text)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .render(area, frame.buffer_mut());
}

fn render_message(frame: &mut Frame, area: Rect, message: &str) {
    Paragraph::new(message)
        .centered()
        .render(area, frame.buffer_mut());
}

pub async fn load_versions(driver: CatalogDriver) -> Result<Vec<VersionData>, anyhow::Error> {
    let launcher = Launcher::builder().build();

    let versions = match driver {
        CatalogDriver::Vanilla => {
            collect_versions(launcher.catalog(VanillaCatalog::with_defaults()).await?).await
        }
        CatalogDriver::Fabric => {
            collect_versions(launcher.catalog(FabricCatalog::with_defaults()).await?).await
        }
        CatalogDriver::Quilt => {
            collect_versions(launcher.catalog(QuiltCatalog::with_defaults()).await?).await
        }
        CatalogDriver::Forge => {
            collect_versions(launcher.catalog(ForgeCatalog::default()).await?).await
        }
        CatalogDriver::NeoForge => {
            collect_versions(launcher.catalog(NeoForgeCatalog::default()).await?).await
        }
        CatalogDriver::Cleanroom => {
            collect_versions(launcher.catalog(CleanroomCatalog::default()).await?).await
        }
        CatalogDriver::LiteLoader => {
            collect_versions(launcher.catalog(LiteLoaderCatalog::with_defaults()).await?).await
        }
        CatalogDriver::Rift => {
            collect_versions(launcher.catalog(RiftCatalog::with_defaults()).await?).await
        }
    };

    Ok(versions)
}

async fn collect_versions<R>(releases: HashMap<MinecraftVersionId, Vec<R>>) -> Vec<VersionData>
where
    R: Release + Send + Sync,
{
    let mut versions = Vec::new();

    for (_game_version, releases) in releases {
        for release in releases {
            let info = release.info().await;
            versions.push(VersionData {
                game_version: info.game_version.to_string(),
                version: info.name,
                description: info.description,
            });
        }
    }

    versions.sort_by(|left, right| {
        left.game_version
            .cmp(&right.game_version)
            .then_with(|| left.version.cmp(&right.version))
    });
    versions
}

#[derive(Debug, Clone, Default)]
pub struct VersionData {
    pub game_version: String,
    pub version: String,
    pub description: Option<String>,
}

impl VersionData {
    fn label(&self) -> String {
        let version = if self.version == self.game_version {
            self.version.clone()
        } else {
            format!("{} / {}", self.game_version, self.version)
        };

        match &self.description {
            Some(description) => format!("{version} - {description}"),
            None => version,
        }
    }
}
