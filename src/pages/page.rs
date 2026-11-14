use crate::t;
use ratail::{KeyHint, NavAction, Page, PageEvent};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Layout, Rect},
};
use unicode_width::UnicodeWidthStr;

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

fn sidebar_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("↑/↓", t!("common.browse")),
        KeyHint::new("ENTER", t!("common.focus")),
    ]
}

pub struct ShellPage<C> {
    focus: Focus,
    content: C,
}

impl<C> ShellPage<C> {
    pub fn new(content: C) -> Self {
        Self {
            focus: Focus::Sidebar,
            content,
        }
    }

    fn on_sidebar_key(&mut self, route: &Route, key: KeyEvent) -> PageEvent<Route> {
        match key.code {
            KeyCode::Down => self.navigate_to(route, route.next()),
            KeyCode::Up => self.navigate_to(route, route.previous()),
            KeyCode::Enter => {
                self.focus = Focus::Content;
                PageEvent::Handled
            }
            _ => PageEvent::Ignored,
        }
    }

    fn navigate_to(&self, current: &Route, target: Route) -> PageEvent<Route> {
        if target == *current {
            PageEvent::Handled
        } else {
            PageEvent::Navigate(NavAction::Replace(target))
        }
    }
}

impl<C: Content + 'static> Page<Services> for ShellPage<C> {
    fn init(&mut self, ctx: &Services, _route: &Route) {
        self.content.init(ctx);
    }

    fn keybindings(&self, _route: &Route) -> Vec<KeyHint> {
        match self.focus {
            Focus::Sidebar => sidebar_hints(),
            Focus::Content => self.content.keybindings(),
        }
    }

    fn on_key(&mut self, ctx: &Services, route: &Route, key: KeyEvent) -> PageEvent<Route> {
        if key.kind == KeyEventKind::Release {
            return PageEvent::Ignored;
        }

        match self.focus {
            Focus::Sidebar => self.on_sidebar_key(route, key),
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

    fn render(&mut self, ctx: &Services, route: &Route, frame: &mut Frame, area: Rect) {
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
                route.index(),
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
        .map(|route| UnicodeWidthStr::width(route.label().as_ref()))
        .max()
        .unwrap_or_default() as u16;

    (longest + 8).clamp(18, 26)
}
