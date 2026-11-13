mod filter;
mod game_versions;
mod loader_versions;

pub use game_versions::load_versions;
pub use loader_versions::load_loader_versions;

use elemental::{core::minecraft::MinecraftVersionId, driver::driver::DriverDescriptor};
use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Tabs, Widget},
};

use self::{
    filter::VersionFilter,
    game_versions::GameVersionsScreen,
    loader_versions::{LoaderSelection, LoaderVersionsScreen},
};
use crate::app::services::Services;
use crate::pages::content::{Content, ContentEvent, container};

const CATALOG_HINTS: &[KeyHint] = &[
    KeyHint::from_static("←/→", "Driver"),
    KeyHint::from_static("Tab", "Filter"),
    KeyHint::from_static("↑/↓", "Scroll"),
    KeyHint::from_static("ENTER", "Loader"),
    KeyHint::from_static("ESC", "Return"),
];

const LOADER_HINTS: &[KeyHint] = &[
    KeyHint::from_static("↑/↓", "Scroll"),
    KeyHint::from_static("ENTER", "Select"),
    KeyHint::from_static("ESC", "Versions"),
];

enum ScreenAction {
    Ignored,
    Handled,
    OpenLoader(MinecraftVersionId),
    Back,
    Selected(LoaderSelection),
}

#[derive(Debug)]
enum Screen {
    GameVersions(GameVersionsScreen),
    LoaderVersions(LoaderVersionsScreen),
}

#[derive(Debug)]
pub struct CatalogContent {
    driver: usize,
    filter: VersionFilter,
    selection: Option<LoaderSelection>,
    screen: Screen,
}

impl Default for CatalogContent {
    fn default() -> Self {
        Self {
            driver: 0,
            filter: VersionFilter::default(),
            selection: None,
            screen: Screen::GameVersions(GameVersionsScreen::default()),
        }
    }
}

impl Content for CatalogContent {
    fn init(&mut self, ctx: &Services) {
        ctx.load_catalog(ctx.drivers()[self.driver]);
    }

    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect) {
        let inner = container(frame, area);
        let [driver_area, secondary_area, body_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(inner);
        render_driver_tabs(frame, driver_area, ctx.drivers(), self.driver);

        let driver = ctx.drivers()[self.driver];
        match &mut self.screen {
            Screen::GameVersions(screen) => {
                render_filter_tabs(frame, secondary_area, self.filter);
                screen.render(ctx, frame, body_area, driver, self.filter);
            }
            Screen::LoaderVersions(screen) => {
                loader_versions::render_header(frame, secondary_area, screen.game());
                screen.render(ctx, frame, body_area, driver, self.selection.as_ref());
            }
        }
    }

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> ContentEvent {
        if matches!(&self.screen, Screen::GameVersions(_)) {
            match key.code {
                KeyCode::Tab => {
                    self.cycle_filter(!key.modifiers.contains(KeyModifiers::SHIFT));
                    return ContentEvent::Handled;
                }
                KeyCode::BackTab => {
                    self.cycle_filter(false);
                    return ContentEvent::Handled;
                }
                KeyCode::Left => {
                    self.switch_driver(ctx, self.driver.saturating_sub(1));
                    return ContentEvent::Handled;
                }
                KeyCode::Right => {
                    let next = self
                        .driver
                        .saturating_add(1)
                        .min(ctx.drivers().len().saturating_sub(1));
                    self.switch_driver(ctx, next);
                    return ContentEvent::Handled;
                }
                _ => {}
            }
        }

        let driver = ctx.drivers()[self.driver];
        let action = match &mut self.screen {
            Screen::GameVersions(screen) => screen.on_key(ctx, key, driver, self.filter),
            Screen::LoaderVersions(screen) => screen.on_key(ctx, key, driver),
        };

        match action {
            ScreenAction::Ignored => ContentEvent::Ignored,
            ScreenAction::Handled => ContentEvent::Handled,
            ScreenAction::OpenLoader(game) => {
                self.screen = Screen::LoaderVersions(LoaderVersionsScreen::new(game.clone()));
                ctx.load_loader_versions(driver, game);
                ContentEvent::Handled
            }
            ScreenAction::Back => {
                self.screen = Screen::GameVersions(GameVersionsScreen::default());
                ContentEvent::Handled
            }
            ScreenAction::Selected(selection) => {
                self.selection = Some(selection);
                ContentEvent::Handled
            }
        }
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        match &self.screen {
            Screen::GameVersions(_) => CATALOG_HINTS.to_vec(),
            Screen::LoaderVersions(_) => LOADER_HINTS.to_vec(),
        }
    }
}

impl CatalogContent {
    fn switch_driver(&mut self, ctx: &Services, driver: usize) {
        if driver == self.driver {
            return;
        }

        self.driver = driver;
        self.selection = None;
        self.screen = Screen::GameVersions(GameVersionsScreen::default());
        ctx.load_catalog(ctx.drivers()[driver]);
    }

    fn cycle_filter(&mut self, forward: bool) {
        let current = VersionFilter::ORDER
            .iter()
            .position(|filter| *filter == self.filter)
            .unwrap_or(0);
        let next = if forward {
            (current + 1) % VersionFilter::ORDER.len()
        } else {
            (current + VersionFilter::ORDER.len() - 1) % VersionFilter::ORDER.len()
        };

        self.filter = VersionFilter::ORDER[next];
        if let Screen::GameVersions(screen) = &mut self.screen {
            screen.reset();
        }
    }
}

fn render_driver_tabs(
    frame: &mut Frame,
    area: Rect,
    drivers: &[DriverDescriptor],
    selected: usize,
) {
    let (start, end) = visible_driver_range(drivers, selected, area.width as usize);
    let titles = drivers[start..end]
        .iter()
        .map(|driver| driver.name)
        .collect::<Vec<&str>>();

    Tabs::new(titles)
        .select(selected - start)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED))
        .divider("│")
        .render(area, frame.buffer_mut());
}

fn render_filter_tabs(frame: &mut Frame, area: Rect, selected: VersionFilter) {
    let titles = VersionFilter::ORDER
        .iter()
        .map(|filter| filter.label())
        .collect::<Vec<&str>>();
    let index = VersionFilter::ORDER
        .iter()
        .position(|filter| *filter == selected)
        .unwrap_or(0);

    Tabs::new(titles)
        .select(index)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED))
        .divider("│")
        .render(area, frame.buffer_mut());
}

fn visible_driver_range(
    drivers: &[DriverDescriptor],
    selected: usize,
    width: usize,
) -> (usize, usize) {
    let widths = drivers
        .iter()
        .map(|driver| driver.name.chars().count() + 2)
        .collect::<Vec<usize>>();
    let mut start = selected;
    let mut end = selected + 1;
    let mut used = widths[selected];
    let mut prefer_left = true;

    loop {
        let mut expanded = false;

        if prefer_left && start > 0 && used + 1 + widths[start - 1] <= width {
            start -= 1;
            used += 1 + widths[start];
            expanded = true;
        } else if !prefer_left && end < widths.len() && used + 1 + widths[end] <= width {
            used += 1 + widths[end];
            end += 1;
            expanded = true;
        } else if start > 0 && used + 1 + widths[start - 1] <= width {
            start -= 1;
            used += 1 + widths[start];
            expanded = true;
        } else if end < widths.len() && used + 1 + widths[end] <= width {
            used += 1 + widths[end];
            end += 1;
            expanded = true;
        }

        if !expanded {
            break;
        }

        prefer_left = !prefer_left;
    }

    (start, end)
}
