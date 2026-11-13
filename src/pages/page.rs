use ratail::{KeyHint, NavAction, Page, PageEvent};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Layout, Rect, Spacing},
};

use crate::app::services::Services;
use crate::pages::content::{Content, ContentEvent};
use crate::pages::route::Route;
use crate::widgets::sidebar::Sidebar;
use crate::widgets::titlebar::TitleBar;

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
            Focus::Content => match key.code {
                KeyCode::Esc => {
                    self.focus = Focus::Sidebar;
                    PageEvent::Handled
                }
                _ => match self.content.on_key(ctx, key) {
                    ContentEvent::Handled => PageEvent::Handled,
                    ContentEvent::Ignored => PageEvent::Ignored,
                },
            },
        }
    }

    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect) {
        let [titlebar_area, body_area] =
            Layout::vertical([Constraint::Max(4), Constraint::Fill(1)])
                .spacing(Spacing::Overlap(1))
                .areas(area);

        let [sidebar_area, content_area] =
            Layout::horizontal([Constraint::Max(20), Constraint::Fill(1)])
                .spacing(Spacing::Overlap(1))
                .areas(body_area);

        let sidebar_items = Route::all()
            .iter()
            .map(|route| route.label().to_string())
            .collect::<Vec<String>>();

        frame.render_widget(TitleBar, titlebar_area);
        frame.render_widget(
            Sidebar::new(sidebar_items, self.route.index()),
            sidebar_area,
        );
        self.content.render(ctx, frame, content_area);
    }
}
