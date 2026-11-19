use crate::t;
use elemental::launcher::{InspectReport, Instance};
use ratail::{KeyHint, SnapshotStatus};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph, Wrap},
};

use crate::drivers;
use crate::pages::content::{Content, ContentEvent, render_loading, render_message};
use crate::services::{LaunchOutcome, Services};
use crate::theme::Theme;
use crate::widgets::{
    confirm::Confirmation,
    list::ListSelection,
    popup::{Popup, PopupEvent},
};

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
    confirm: Option<Confirmation<String>>,
}

impl Content for InstancesContent {
    fn init(&mut self, ctx: &Services) {
        ctx.instances().load();
    }

    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect) {
        let theme = ctx.ui().theme();
        let [body_area, status_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(area);

        let snapshot = ctx.instances().snapshot();
        let view = snapshot.view();
        match (view.data, view.error) {
            (Some(report), Some(error)) => {
                let [error_area, report_area] =
                    Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(body_area);
                render_message(
                    frame,
                    error_area,
                    t!("common.error", error = error).as_ref(),
                );
                self.render_report(report, frame, report_area, theme);
            }
            (Some(report), None) => self.render_report(report, frame, body_area, theme),
            (None, Some(error)) => {
                render_message(frame, body_area, t!("common.error", error = error).as_ref())
            }
            (None, None) if view.status == SnapshotStatus::Cancelled => {
                render_message(frame, body_area, t!("common.cancelled").as_ref())
            }
            (None, None) => render_loading(ctx, frame, body_area, t!("instances.loading").as_ref()),
        }

        render_launch_status(ctx, frame, status_area, theme);

        if let Some(confirm) = &self.confirm {
            let anchor = Rect {
                x: area.x,
                y: area.bottom().saturating_sub(1),
                width: 1,
                height: 1,
            };
            confirm.render(frame, area, anchor, theme);
        }
    }

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> ContentEvent {
        if let Some(confirm) = &mut self.confirm {
            match confirm.handle_key(key) {
                PopupEvent::Ignored => return ContentEvent::Ignored,
                PopupEvent::Handled => return ContentEvent::Handled,
                PopupEvent::Cancelled => {
                    self.confirm = None;
                    return ContentEvent::Handled;
                }
                PopupEvent::Completed(instance_name) => {
                    self.confirm = None;
                    ctx.instances().remove(instance_name);
                    return ContentEvent::Handled;
                }
            }
        }

        match key.code {
            KeyCode::Down | KeyCode::Up | KeyCode::Home | KeyCode::End => {
                self.list.handle_key(key.code);
                ContentEvent::Handled
            }
            KeyCode::Enter => {
                if let Some(instance) = self.selected_instance(ctx) {
                    ctx.launch().launch_instance(instance);
                }
                ContentEvent::Handled
            }
            KeyCode::Char('r') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                ctx.instances().load();
                ContentEvent::Handled
            }
            KeyCode::Char('d') | KeyCode::Delete
                if !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(instance_name) = self.selected_instance(ctx) {
                    self.confirm = Some(Confirmation::open(
                        t!("instances.confirm_remove", instance = instance_name.clone())
                            .into_owned(),
                        instance_name,
                    ));
                }
                ContentEvent::Handled
            }
            _ => ContentEvent::Ignored,
        }
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        match &self.confirm {
            Some(confirm) => confirm.hints(),
            None => instance_hints(),
        }
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
