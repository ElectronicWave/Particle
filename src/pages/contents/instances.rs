use crate::t;
use elemental::launcher::{InspectReport, Instance};
use ratail::{KeyHint, ModalHost, PageEvent, SnapshotStatus, WithModal};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph, Wrap},
};

use crate::drivers;
use crate::pages::content::{render_message, render_snapshot};
use crate::pages::route::Route;
use crate::services::{LaunchOutcome, Services};
use crate::theme::Theme;
use crate::widgets::{confirm::Confirmation, list::ListSelection};

fn instance_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("↑/↓", t!("common.browse")),
        KeyHint::new("ENTER", t!("common.launch")),
        KeyHint::new("D", t!("common.remove")),
        KeyHint::new("R", t!("common.refresh")),
        KeyHint::new("ESC", t!("common.return")),
    ]
}

#[derive(Debug, Default)]
pub struct InstancesContent {
    list: ListSelection,
}

impl WithModal<Services> for InstancesContent {
    fn init(&mut self, ctx: &Services, _route: &Route, _modals: &mut ModalHost<Services, Self>) {
        ctx.instances().load();
    }

    fn render(&mut self, ctx: &Services, _route: &Route, frame: &mut Frame, area: Rect) {
        let theme = ctx.ui().theme();
        let [body_area, status_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(area);

        let snapshot = ctx.instances().snapshot();
        render_snapshot(
            ctx,
            &snapshot.view(),
            t!("instances.loading").as_ref(),
            frame,
            body_area,
            |report, frame, area| self.render_report(report, frame, area, theme),
        );
        render_launch_status(ctx, frame, status_area, theme);
    }

    fn on_key(
        &mut self,
        ctx: &Services,
        _route: &Route,
        modals: &mut ModalHost<Services, Self>,
        key: KeyEvent,
    ) -> PageEvent<Route> {
        match key.code {
            KeyCode::Down | KeyCode::Up | KeyCode::Home | KeyCode::End => {
                self.list.handle_key(key.code);
                PageEvent::Handled
            }
            KeyCode::Enter => {
                if let Some(instance) = self.selected_instance(ctx) {
                    ctx.launch().launch_instance(instance);
                }
                PageEvent::Handled
            }
            KeyCode::Char('r') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                ctx.instances().load();
                PageEvent::Handled
            }
            KeyCode::Char('d') | KeyCode::Delete
                if !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(instance_name) = self.selected_instance(ctx) {
                    let title = t!("instances.confirm_remove", instance = instance_name.clone())
                        .into_owned();
                    modals.open(
                        Confirmation::open(title, instance_name),
                        |_page, ctx, _modals, name| {
                            ctx.instances().remove(name);
                        },
                    );
                }
                PageEvent::Handled
            }
            _ => PageEvent::Ignored,
        }
    }

    fn keybindings(&self, _route: &Route) -> Vec<KeyHint> {
        instance_hints()
    }
}

impl InstancesContent {
    fn render_report(
        &mut self,
        report: &InspectReport,
        frame: &mut Frame,
        area: Rect,
        theme: Theme,
    ) {
        let (list_area, failures_area) = if report.failures.is_empty() {
            (area, None)
        } else {
            let visible = report.failures.len().min(4);
            let height = visible as u16 + 1;
            let [list_area, failures_area] =
                Layout::vertical([Constraint::Fill(1), Constraint::Length(height)]).areas(area);
            (list_area, Some(failures_area))
        };

        self.render_instances(&report.instances, frame, list_area, theme);
        if let Some(failures_area) = failures_area {
            render_failures(report, frame, failures_area, theme);
        }
    }

    fn render_instances(
        &mut self,
        instances: &[Instance],
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
                let driver_name = drivers::display_name(instance.driver.driver);
                let line = Line::from(vec![
                    Span::styled(
                        instance.instance_name.clone(),
                        Style::new().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  {driver_name} {version}"),
                        Style::new().fg(theme.text_dim),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect::<Vec<ListItem>>();
        let list = List::new(items).highlight_style(theme.selection_style());

        frame.render_stateful_widget(list, area, self.list.state_mut());
    }

    fn selected_instance(&self, ctx: &Services) -> Option<String> {
        let snapshot = ctx.instances().snapshot();
        let report = snapshot.view().data?;
        let selected = self.list.selected()?;
        report
            .instances
            .get(selected)
            .map(|instance| instance.instance_name.clone())
    }
}

fn render_failures(report: &InspectReport, frame: &mut Frame, area: Rect, theme: Theme) {
    let mut lines = vec![Line::styled(
        t!("instances.failures", count = report.failures.len()),
        Style::new().fg(theme.warning),
    )];
    let visible = report.failures.len().min(4);
    for failure in &report.failures[..visible] {
        lines.push(Line::styled(
            t!(
                "instances.failure",
                instance = failure.instance_name.as_str(),
                path = failure.path.display().to_string(),
                error = failure.error.to_string()
            ),
            Style::new().fg(theme.danger),
        ));
    }
    if report.failures.len() > visible {
        lines.push(Line::styled(
            t!(
                "instances.failures_more",
                count = report.failures.len() - visible
            ),
            Style::new().fg(theme.text_dim),
        ));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_launch_status(ctx: &Services, frame: &mut Frame, area: Rect, theme: Theme) {
    let snapshot = ctx.launch().snapshot();
    let view = snapshot.view();
    let lines = if let Some(error) = view.error {
        vec![Line::styled(
            t!("instances.failed", error = format!("{error:#}")),
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
