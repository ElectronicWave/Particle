use crate::t;
use ratail::{HitMap, InputEvent, KeyChord, KeyHint, MouseKind, NavAction, Page, PageEvent};
use ratatui::{
    Frame,
    crossterm::event::KeyCode,
    layout::{Constraint, Layout, Rect},
    widgets::{Block, Borders, Padding},
};
use unicode_width::UnicodeWidthStr;

use crate::pages::route::Route;
use crate::services::Services;
use crate::widgets::panel;
use crate::widgets::sidebar::Sidebar;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Focus {
    Sidebar,
    Content,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ShellTarget {
    Sidebar,
    Route(Route),
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
    hits: HitMap<ShellTarget>,
}

impl<C> ShellPage<C> {
    pub fn new(content: C) -> Self {
        Self {
            focus: Focus::Sidebar,
            content,
            hits: HitMap::new(),
        }
    }

    fn on_sidebar_key(&mut self, route: &Route, key: KeyChord) -> PageEvent<Route> {
        match key.code {
            KeyCode::Down => self.navigate_to(route, route.next()),
            KeyCode::Up => self.navigate_to(route, route.previous()),
            KeyCode::Enter | KeyCode::Right => {
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

impl<C: Page<Services> + 'static> Page<Services> for ShellPage<C> {
    fn init(&mut self, ctx: &Services, route: &Route) {
        self.content.init(ctx, route);
    }

    fn resume(&mut self, ctx: &Services, route: &Route) {
        self.content.resume(ctx, route);
    }

    fn pause(&mut self, ctx: &Services, route: &Route) {
        self.content.pause(ctx, route);
    }

    fn dispose(&mut self, ctx: &Services, route: &Route) {
        self.content.dispose(ctx, route);
    }

    fn keybindings(&self, route: &Route) -> Vec<KeyHint> {
        match self.focus {
            Focus::Sidebar => sidebar_hints(),
            Focus::Content => self.content.keybindings(route),
        }
    }

    fn on_input(&mut self, ctx: &Services, route: &Route, input: InputEvent) -> PageEvent<Route> {
        if let InputEvent::Mouse(mouse) = input {
            let Some(hit) = self.hits.hit_mouse_copied(mouse) else {
                return PageEvent::Ignored;
            };
            return match hit.id {
                ShellTarget::Route(target) => match mouse.kind {
                    MouseKind::Click | MouseKind::DoubleClick => {
                        self.focus = Focus::Sidebar;
                        self.navigate_to(route, target)
                    }
                    MouseKind::ScrollDown => self.navigate_to(route, route.next()),
                    MouseKind::ScrollUp => self.navigate_to(route, route.previous()),
                    _ => PageEvent::Ignored,
                },
                ShellTarget::Sidebar => match mouse.kind {
                    MouseKind::Click => {
                        self.focus = Focus::Sidebar;
                        PageEvent::Handled
                    }
                    MouseKind::ScrollDown => self.navigate_to(route, route.next()),
                    MouseKind::ScrollUp => self.navigate_to(route, route.previous()),
                    _ => PageEvent::Ignored,
                },
                ShellTarget::Content => {
                    let focus_changed = self.focus != Focus::Content
                        && matches!(
                            mouse.kind,
                            MouseKind::Click
                                | MouseKind::DoubleClick
                                | MouseKind::RightClick
                                | MouseKind::ScrollUp
                                | MouseKind::ScrollDown
                        );
                    if focus_changed {
                        self.focus = Focus::Content;
                    }
                    match self.content.on_input(ctx, route, InputEvent::Mouse(mouse)) {
                        PageEvent::Ignored if focus_changed => PageEvent::Handled,
                        event => event,
                    }
                }
            };
        }

        match self.focus {
            Focus::Sidebar => match input {
                InputEvent::Key(key) => self.on_sidebar_key(route, key),
                InputEvent::Paste(_) | InputEvent::Mouse(_) => PageEvent::Ignored,
            },
            Focus::Content => {
                let is_esc = input == InputEvent::Key(KeyChord::plain(KeyCode::Esc));
                match self.content.on_input(ctx, route, input) {
                    PageEvent::Ignored if is_esc => {
                        self.focus = Focus::Sidebar;
                        PageEvent::Handled
                    }
                    event => event,
                }
            }
        }
    }

    fn poll(&mut self, ctx: &Services, route: &Route) -> Option<NavAction<Route>> {
        self.content.poll(ctx, route)
    }

    fn render(&mut self, ctx: &Services, route: &Route, frame: &mut Frame, area: Rect) {
        self.hits.clear();
        let theme = ctx.ui().theme();
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
        self.hits.register(sidebar_area, ShellTarget::Sidebar);
        let sidebar_inner = Block::default()
            .borders(Borders::ALL)
            .padding(Padding::horizontal(1))
            .inner(sidebar_area);
        for (index, item_route) in Route::all().iter().enumerate() {
            if (index as u16) >= sidebar_inner.height {
                break;
            }
            let row = Rect::new(
                sidebar_inner.x,
                sidebar_inner.y + index as u16,
                sidebar_inner.width,
                1,
            );
            self.hits.register(row, ShellTarget::Route(*item_route));
        }

        self.hits.register(content_area, ShellTarget::Content);
        let inner = panel::render(frame, content_area, theme, self.focus == Focus::Content);
        self.content.render(ctx, route, frame, inner);
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
