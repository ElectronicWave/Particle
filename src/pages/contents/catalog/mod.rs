mod filter;
mod game_versions;
mod instance_name;
mod loader_versions;

pub use game_versions::load_versions;
pub use loader_versions::load_loader_versions;

use crate::t;
use elemental::{
    core::minecraft::MinecraftVersionId,
    driver::{descriptors::VANILLA_DRIVER, driver::DriverDescriptor},
};
use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use self::{
    filter::VersionFilter,
    game_versions::GameVersionsScreen,
    instance_name::{InstanceNameScreen, NameAction},
    loader_versions::{LoaderSelection, LoaderVersionsScreen},
};
use crate::app::services::Services;
use crate::pages::content::{Content, ContentEvent};
use crate::pages::route::Route;
use crate::theme::Theme;
use crate::widgets::cycle::Cycle;
use crate::widgets::tabs::TabBar;

fn catalog_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("←/→", t!("common.driver")),
        KeyHint::new("Tab", t!("common.filter")),
        KeyHint::new("↑/↓", t!("common.scroll")),
        KeyHint::new("ENTER", t!("common.select")),
        KeyHint::new("ESC", t!("common.return")),
    ]
}

fn loader_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("↑/↓", t!("common.scroll")),
        KeyHint::new("ENTER", t!("common.select")),
        KeyHint::new("ESC", t!("common.versions")),
    ]
}

fn name_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("ENTER", t!("common.install")),
        KeyHint::new("ESC", t!("common.back")),
    ]
}

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
    InstanceName(InstanceNameScreen),
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
        let theme = ctx.theme();
        let [driver_area, secondary_area, body_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(area);
        render_driver_tabs(frame, driver_area, ctx.drivers(), self.driver, theme);

        let driver = ctx.drivers()[self.driver];
        match &mut self.screen {
            Screen::GameVersions(screen) => {
                render_filter_tabs(frame, secondary_area, self.filter, theme);
                screen.render(ctx, frame, body_area, driver, self.filter);
            }
            Screen::LoaderVersions(screen) => {
                loader_versions::render_header(frame, secondary_area, screen.game(), theme);
                screen.render(ctx, frame, body_area, driver, self.selection.as_ref());
            }
            Screen::InstanceName(screen) => {
                if let Some(selection) = &self.selection {
                    render_flow_header(
                        frame,
                        secondary_area,
                        &selection.game.to_string(),
                        &t!("catalog.instance_name_suffix"),
                        theme,
                    );
                }
                screen.render(frame, body_area, theme);
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
        match &mut self.screen {
            Screen::GameVersions(screen) => {
                let action = screen.on_key(ctx, key, driver, self.filter);
                match action {
                    ScreenAction::Ignored => ContentEvent::Ignored,
                    ScreenAction::Handled => ContentEvent::Handled,
                    ScreenAction::OpenLoader(game) => {
                        if driver == VANILLA_DRIVER {
                            self.open_instance_name(LoaderSelection { game, loader: None });
                        } else {
                            self.screen =
                                Screen::LoaderVersions(LoaderVersionsScreen::new(game.clone()));
                            ctx.load_loader_versions(driver, game);
                        }
                        ContentEvent::Handled
                    }
                    ScreenAction::Back | ScreenAction::Selected(_) => ContentEvent::Handled,
                }
            }
            Screen::LoaderVersions(screen) => {
                let action = screen.on_key(ctx, key, driver);
                match action {
                    ScreenAction::Ignored => ContentEvent::Ignored,
                    ScreenAction::Handled => ContentEvent::Handled,
                    ScreenAction::Back => {
                        self.screen = Screen::GameVersions(GameVersionsScreen::default());
                        ContentEvent::Handled
                    }
                    ScreenAction::Selected(selection) => {
                        self.open_instance_name(selection);
                        ContentEvent::Handled
                    }
                    ScreenAction::OpenLoader(_) => ContentEvent::Handled,
                }
            }
            Screen::InstanceName(screen) => match screen.on_key(key) {
                NameAction::Ignored => ContentEvent::Ignored,
                NameAction::Handled => ContentEvent::Handled,
                NameAction::Cancel => {
                    self.leave_instance_name();
                    ContentEvent::Handled
                }
                NameAction::Submit(instance_name) => {
                    if let Some(selection) = &self.selection {
                        ctx.start_install(
                            instance_name,
                            driver,
                            selection.game.clone(),
                            selection.loader.clone(),
                        );
                        return ContentEvent::Navigate(Route::Downloads);
                    }
                    ContentEvent::Handled
                }
            },
        }
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        match &self.screen {
            Screen::GameVersions(_) => catalog_hints(),
            Screen::LoaderVersions(_) => loader_hints(),
            Screen::InstanceName(_) => name_hints(),
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
        if let Some(filter) = Cycle::new(&VersionFilter::ORDER).step(&self.filter, forward) {
            self.filter = *filter;
        }
        if let Screen::GameVersions(screen) = &mut self.screen {
            screen.reset();
        }
    }

    fn open_instance_name(&mut self, selection: LoaderSelection) {
        let default = default_instance_name(&selection);
        self.selection = Some(selection);
        self.screen = Screen::InstanceName(InstanceNameScreen::new(default));
    }

    fn leave_instance_name(&mut self) {
        match self.selection.take() {
            Some(selection) if selection.loader.is_some() => {
                let game = selection.game.clone();
                self.selection = Some(selection);
                self.screen = Screen::LoaderVersions(LoaderVersionsScreen::new(game));
            }
            _ => {
                self.screen = Screen::GameVersions(GameVersionsScreen::default());
            }
        }
    }
}

fn default_instance_name(selection: &LoaderSelection) -> String {
    match &selection.loader {
        Some(loader) => format!("{}-{}", selection.game, loader),
        None => selection.game.to_string(),
    }
}

fn render_flow_header(frame: &mut Frame, area: Rect, label: &str, suffix: &str, theme: Theme) {
    let line = Line::from(vec![
        Span::styled(label.to_owned(), Style::new().add_modifier(Modifier::BOLD)),
        Span::styled(format!("  ›  {suffix}"), Style::new().fg(theme.text_dim)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_driver_tabs(
    frame: &mut Frame,
    area: Rect,
    drivers: &[DriverDescriptor],
    selected: usize,
    theme: Theme,
) {
    let titles = drivers
        .iter()
        .map(|driver| driver.name)
        .collect::<Vec<&str>>();
    frame.render_widget(TabBar::new(theme, &titles, selected), area);
}

fn render_filter_tabs(frame: &mut Frame, area: Rect, selected: VersionFilter, theme: Theme) {
    let titles = VersionFilter::ORDER
        .iter()
        .map(|filter| filter.label().into_owned())
        .collect::<Vec<String>>();
    let labels = titles.iter().map(String::as_str).collect::<Vec<&str>>();
    let index = VersionFilter::ORDER
        .iter()
        .position(|filter| *filter == selected)
        .unwrap_or(0);

    frame.render_widget(TabBar::new(theme, &labels, index).secondary(), area);
}
