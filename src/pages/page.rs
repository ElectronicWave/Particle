use ratail::{KeyHint, NavAction, Page, PageEvent};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Layout, Rect},
};

use crate::app::services::Services;
use crate::pages::content::{Content, ContentEvent};
use crate::pages::route::Route;
use crate::widgets::panel;
use crate::widgets::sidebar::Sidebar;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Focus {
    Sidebar,
    Content,
}

const SIDEBAR_HINTS: &[KeyHint] = &[
    KeyHint::from_static("↑/↓", "Browse"),
    KeyHint::from_static("ENTER", "Focus"),
];

pub struct ShellPage<C> {
    route: Route,
    focus: Focus,
    content: C,
}

impl<C> ShellPage<C> {
    pub fn new(route: Route, content: C) -> Self {
        Self {
            route,
            focus: Focus::Sidebar,
            content,
        }
    }

    fn on_sidebar_key(&mut self, key: KeyEvent) -> PageEvent<Route> {
        match key.code {
            KeyCode::Down => self.navigate_to(self.route.next()),
            KeyCode::Up => self.navigate_to(self.route.previous()),
            KeyCode::Enter => {
                self.focus = Focus::Content;
                PageEvent::Handled
            }
            _ => PageEvent::Ignored,
        }
    }

    fn navigate_to(&self, route: Route) -> PageEvent<Route> {
        if route == self.route {
            PageEvent::Handled
        } else {
            PageEvent::Navigate(NavAction::Replace(route))
        }
    }
}

impl<C: Content + 'static> Page<Services> for ShellPage<C> {
    fn init(&mut self, ctx: &Services) {
        self.content.init(ctx);
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        match self.focus {
            Focus::Sidebar => SIDEBAR_HINTS.to_vec(),
            Focus::Content => self.content.keybindings(),
        }
    }

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> PageEvent<Route> {
        if key.kind == KeyEventKind::Release {
            return PageEvent::Ignored;
        }

        match self.focus {
            Focus::Sidebar => self.on_sidebar_key(key),
            Focus::Content => match self.content.on_key(ctx, key) {
                ContentEvent::Handled => PageEvent::Handled,
                ContentEvent::Navigate(route) => PageEvent::Navigate(NavAction::Replace(route)),
                ContentEvent::Ignored if key.code == KeyCode::Esc => {
                    self.focus = Focus::Sidebar;
                    PageEvent::Handled
                }
                ContentEvent::Ignored => PageEvent::Ignored,
            },
        }
    }

    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect) {
        let theme = ctx.theme();
        let [sidebar_area, _gutter, content_area] = Layout::horizontal([
            Constraint::Length(sidebar_width()),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(area);

        let sidebar_items = Route::all()
            .iter()
            .map(|route| route.label().to_string())
            .collect::<Vec<String>>();

        frame.render_widget(
            Sidebar::new(
                sidebar_items,
                self.route.index(),
                self.focus == Focus::Sidebar,
                theme,
            ),
            sidebar_area,
        );
        let inner = panel::render(frame, content_area, theme, self.focus == Focus::Content);
        self.content.render(ctx, frame, inner);
    }
}

fn sidebar_width() -> u16 {
    let longest = Route::all()
        .iter()
        .map(|route| route.label().chars().count())
        .max()
        .unwrap_or_default() as u16;

    (longest + 8).clamp(18, 26)
}
