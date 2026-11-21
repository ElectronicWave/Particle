mod filter;
mod game_versions;
mod instance_name;
mod loader_versions;

use crate::t;
use elemental::{
    core::minecraft::MinecraftVersionId,
    driver::{driver::DriverDescriptor, loader_version::LoaderVersionId},
};
use ratail::{KeyHint, NavAction, Navigator, Page, PageEvent, Router};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use self::filter::VersionFilter;
use self::game_versions::GameVersionsPage;
use self::instance_name::InstanceNamePage;
use self::loader_versions::LoaderVersionsPage;
use crate::drivers as driver_display;
use crate::pages::route::Route;
use crate::services::{InstallSessionId, Services};
use crate::theme::Theme;
use crate::widgets::cycle::Cycle;
use crate::widgets::tabs::{TabBar, cycle_index, tab_step};

#[derive(Clone, Debug, PartialEq, Eq)]
struct LoaderSelection {
    game: MinecraftVersionId,
    loader: Option<LoaderVersionId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CatalogRoute {
    GameVersions {
        driver: usize,
        filter: VersionFilter,
    },
    LoaderVersions {
        driver: usize,
        game: MinecraftVersionId,
        filter: VersionFilter,
    },
    /// The final installation step: name the instance and start the install.
    InstanceName {
        driver: usize,
        selection: LoaderSelection,
    },
}

struct CatalogRouter;

impl Router<Services, CatalogRoute> for CatalogRouter {
    fn create(
        &mut self,
        route: &CatalogRoute,
        _ctx: &Services,
    ) -> Box<dyn Page<Services, CatalogRoute>> {
        match route {
            CatalogRoute::GameVersions { .. } => Box::new(GameVersionsPage::default()),
            CatalogRoute::LoaderVersions { .. } => Box::new(LoaderVersionsPage::default()),
            CatalogRoute::InstanceName { driver, selection } => {
                Box::new(InstanceNamePage::new(*driver, selection.clone()))
            }
        }
    }
}

fn catalog_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("TAB", t!("common.switch_tab")),
        KeyHint::new("←/→", t!("common.filter")),
        KeyHint::new("↑/↓", t!("common.scroll")),
        KeyHint::new("ENTER", t!("common.select")),
        KeyHint::new("ESC", t!("common.return")),
    ]
}

fn loader_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("TAB", t!("common.switch_tab")),
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

pub struct CatalogContent {
    nav: Navigator<Services, CatalogRoute, CatalogRouter>,
    seen_session: Option<InstallSessionId>,
}

impl CatalogContent {
    pub fn new(ctx: &Services) -> Self {
        Self {
            nav: Navigator::new(
                CatalogRouter,
                CatalogRoute::GameVersions {
                    driver: 0,
                    filter: VersionFilter::default(),
                },
                ctx,
            ),
            seen_session: ctx.install().latest_session(),
        }
    }

    fn current_driver(&self) -> Option<usize> {
        match self.nav.current_route()? {
            CatalogRoute::GameVersions { driver, .. }
            | CatalogRoute::LoaderVersions { driver, .. }
            | CatalogRoute::InstanceName { driver, .. } => Some(*driver),
        }
    }

    fn cycle_filter(filter: VersionFilter, forward: bool) -> VersionFilter {
        Cycle::new(&VersionFilter::ORDER)
            .step(&filter, forward)
            .copied()
            .unwrap_or(filter)
    }

    fn handle_header(&mut self, ctx: &Services, key: KeyEvent) -> Option<PageEvent<Route>> {
        let (driver, filter, game_versions) = match self.nav.current_route()? {
            CatalogRoute::GameVersions { driver, filter } => (*driver, *filter, true),
            CatalogRoute::LoaderVersions { driver, filter, .. } => (*driver, *filter, false),
            CatalogRoute::InstanceName { .. } => return None,
        };

        if let Some(step) = tab_step(key) {
            let next = cycle_index(ctx.catalog().drivers().len(), driver, step);
            if next == driver {
                return Some(PageEvent::Handled);
            }
            self.nav.reset(
                CatalogRoute::GameVersions {
                    driver: next,
                    filter,
                },
                ctx,
            );
            return Some(PageEvent::Handled);
        }

        if !game_versions {
            return None;
        }
        if !matches!(key.code, KeyCode::Left | KeyCode::Right) {
            return None;
        }
        let filter = Self::cycle_filter(filter, key.code == KeyCode::Right);
        self.nav
            .reset(CatalogRoute::GameVersions { driver, filter }, ctx);
        Some(PageEvent::Handled)
    }
}

impl Page<Services> for CatalogContent {
    fn resume(&mut self, ctx: &Services, _route: &Route) {
        if self.seen_session.is_none() {
            self.seen_session = ctx.install().latest_session();
        }
        self.nav.resume_current(ctx);
    }

    fn pause(&mut self, ctx: &Services, _route: &Route) {
        self.nav.pause_current(ctx);
    }

    fn dispose(&mut self, ctx: &Services, _route: &Route) {
        self.nav.dispose_all(ctx);
    }

    fn render(&mut self, ctx: &Services, _route: &Route, frame: &mut Frame, area: Rect) {
        let theme = ctx.ui().theme();
        let [driver_area, body_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);
        let driver = self.current_driver().unwrap_or_default();
        render_driver_tabs(frame, driver_area, ctx.catalog().drivers(), driver, theme);
        self.nav.render(ctx, frame, body_area);
    }

    fn on_key(&mut self, ctx: &Services, _route: &Route, key: KeyEvent) -> PageEvent<Route> {
        if let Some(event) = self.handle_header(ctx, key) {
            return event;
        }
        if self.nav.handle_key(ctx, key) {
            PageEvent::Handled
        } else {
            PageEvent::Ignored
        }
    }

    fn poll(&mut self, ctx: &Services, _route: &Route) -> Option<NavAction<Route>> {
        let latest = ctx.install().latest_session();
        if latest != self.seen_session {
            self.seen_session = latest;
            if latest.is_some() {
                return Some(NavAction::Replace(Route::Downloads));
            }
        }
        self.nav.poll_current(ctx);
        None
    }

    fn keybindings(&self, _route: &Route) -> Vec<KeyHint> {
        self.nav.keybindings()
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
        .map(|driver| driver_display::display_name(*driver))
        .collect::<Vec<_>>();
    let labels = titles.iter().map(AsRef::as_ref).collect::<Vec<&str>>();
    frame.render_widget(TabBar::new(theme, &labels, selected), area);
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
