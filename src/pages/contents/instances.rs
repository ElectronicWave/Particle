use crate::t;
use ratail::{KeyHint, SnapshotStatus};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};

use crate::app::services::{LaunchOutcome, Services};
use crate::pages::content::{Content, ContentEvent, render_loading, render_message};
use crate::theme::Theme;

fn instance_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("↑/↓", t!("common.browse")),
        KeyHint::new("ENTER", t!("common.launch")),
        KeyHint::new("R", t!("common.refresh")),
        KeyHint::new("ESC", t!("common.return")),
    ]
}

#[derive(Debug, Default)]
pub struct InstancesContent {
    list: ListState,
}

impl Content for InstancesContent {
    fn init(&mut self, ctx: &Services) {
        ctx.load_instances();
    }

    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect) {
        if self.list.selected().is_none() {
            self.list.select(Some(0));
        }

        let theme = ctx.theme();
        let [body_area, status_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(area);

        let snapshot = ctx.instances().snapshot();
        let view = snapshot.view();
        match (view.data, view.error) {
            (Some(instances), Some(error)) => {
                let [error_area, list_area] =
                    Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(body_area);
                render_message(
                    frame,
                    error_area,
                    t!("common.error", error = error).as_ref(),
                );
                self.render_instances(instances, frame, list_area, theme);
            }
            (Some(instances), None) => self.render_instances(instances, frame, body_area, theme),
            (None, Some(error)) => {
                render_message(frame, body_area, t!("common.error", error = error).as_ref())
            }
            (None, None) if view.status == SnapshotStatus::Cancelled => {
                render_message(frame, body_area, t!("common.cancelled").as_ref())
            }
            (None, None) => render_loading(ctx, frame, body_area, t!("instances.loading").as_ref()),
        }

        render_launch_status(ctx, frame, status_area, theme);
    }

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> ContentEvent {
        match key.code {
            KeyCode::Down => {
                self.list.select_next();
                ContentEvent::Handled
            }
            KeyCode::Up => {
                self.list.select_previous();
                ContentEvent::Handled
            }
            KeyCode::Enter => {
                if let Some(instance) = self.selected_instance(ctx) {
                    ctx.launch_instance(instance);
                }
                ContentEvent::Handled
            }
            KeyCode::Char('r') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                ctx.load_instances();
                ContentEvent::Handled
            }
            _ => ContentEvent::Ignored,
        }
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        instance_hints()
    }
}

impl InstancesContent {
    fn render_instances(
        &mut self,
        instances: &[elemental::launcher::Instance],
        frame: &mut Frame,
        area: Rect,
        theme: Theme,
    ) {
        if instances.is_empty() {
            render_message(frame, area, t!("instances.empty").as_ref());
            return;
        }

        let items = instances
            .iter()
            .map(|instance| {
                let version = instance
                    .driver
                    .game_version
                    .as_ref()
                    .map(|version| version.to_string())
                    .unwrap_or_else(|| t!("common.unknown_version").into_owned());
                let line = Line::from(vec![
                    Span::styled(
                        instance.instance_name.clone(),
                        Style::new().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  {} {version}", instance.driver.driver.name),
                        Style::new().fg(theme.text_dim),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect::<Vec<ListItem>>();
        let list = List::new(items).highlight_style(theme.selection_style());

        frame.render_stateful_widget(list, area, &mut self.list);
    }

    fn selected_instance(&self, ctx: &Services) -> Option<String> {
        let snapshot = ctx.instances().snapshot();
        let instances = snapshot.view().data?;
        let selected = self.list.selected()?;
        instances
            .get(selected)
            .map(|instance| instance.instance_name.clone())
    }
}

fn render_launch_status(ctx: &Services, frame: &mut Frame, area: Rect, theme: Theme) {
    let snapshot = ctx.launch_task().snapshot();
    let view = snapshot.view();
    let lines = if let Some(error) = view.error {
        vec![Line::styled(
            t!("instances.failed", error = error),
            Style::new().fg(theme.danger),
        )]
    } else if view.status == SnapshotStatus::Loading {
        let line = match view.data.map(|outcome| outcome.as_ref()) {
            Some(LaunchOutcome::Started(report)) => t!(
                "instances.launching_last",
                instance = report.instance_name.as_str()
            ),
            _ => t!("instances.launching"),
        };
        vec![Line::styled(line, Style::new().fg(theme.accent))]
    } else {
        match view.data.map(|outcome| outcome.as_ref()) {
            Some(LaunchOutcome::Started(report)) => vec![
                Line::styled(
                    t!(
                        "instances.launched",
                        instance = report.instance_name.as_str(),
                        executable = report.executable.display()
                    ),
                    Style::new().fg(theme.success),
                ),
                Line::styled(
                    t!("instances.log", path = report.log_path.display()),
                    Style::new().fg(theme.text_dim),
                ),
            ],
            Some(LaunchOutcome::NoAccount) => vec![Line::styled(
                t!("instances.no_account"),
                Style::new().fg(theme.warning),
            )],
            Some(LaunchOutcome::InstanceNotFound(instance)) => vec![Line::styled(
                t!("instances.not_found", instance = instance.as_str()),
                Style::new().fg(theme.danger),
            )],
            None if view.status == SnapshotStatus::Cancelled => vec![Line::styled(
                t!("instances.cancelled"),
                Style::new().fg(theme.text_dim),
            )],
            None => vec![Line::styled(
                t!("instances.hint"),
                Style::new().fg(theme.text_dim),
            )],
        }
    };

    frame.render_widget(Paragraph::new(lines), area);
}
