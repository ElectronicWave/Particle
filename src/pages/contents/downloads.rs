use std::fmt::Write;

use crate::t;
use elemental::infra::downloader::DownloadProgress;
use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph, Wrap},
};

use crate::app::services::Services;
use crate::pages::content::{Content, ContentEvent};
use crate::states::{InstallPhase, InstallState};
use crate::theme::Theme;

fn download_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("ENTER", t!("common.dismiss")),
        KeyHint::new("ESC", t!("common.return")),
    ]
}

#[derive(Debug, Default)]
pub struct DownloadsContent;

impl Content for DownloadsContent {
    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect) {
        let theme = ctx.theme();
        let [title_area, body_area] =
            Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(area);

        frame.render_widget(
            Paragraph::new(Line::styled(
                t!("downloads.title"),
                Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
            )),
            title_area,
        );

        render_state(frame, body_area, &ctx.install_state(), theme);
    }

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> ContentEvent {
        if key.code != KeyCode::Enter {
            return ContentEvent::Ignored;
        }

        let state = ctx.install_state();
        if !state.phase.is_preparing() {
            ctx.reset_install();
        }
        ContentEvent::Handled
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        download_hints()
    }
}

fn render_state(frame: &mut Frame, area: Rect, state: &InstallState, theme: Theme) {
    match &state.phase {
        InstallPhase::Idle => {
            render_message(frame, area, t!("downloads.empty").as_ref());
        }
        InstallPhase::Preparing => render_progress(frame, area, state, theme),
        InstallPhase::Done => {
            render_message(
                frame,
                area,
                t!("downloads.done", instance = state.instance_name.as_str()).as_ref(),
            );
        }
        InstallPhase::Failed(error) => {
            let lines = vec![
                Line::styled(t!("downloads.failed"), Style::new().fg(theme.danger).bold()),
                Line::raw(""),
                Line::raw(error.clone()),
                Line::raw(""),
                Line::styled(t!("downloads.dismiss"), Style::new().fg(theme.text_dim)),
            ];
            frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
        }
    }
}

fn render_progress(frame: &mut Frame, area: Rect, state: &InstallState, theme: Theme) {
    let [header_area, gauge_area, stats_area, tasks_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Fill(1),
    ])
    .areas(area);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(t!("downloads.installing"), Style::new().fg(theme.accent)),
            Span::raw(" "),
            Span::styled(
                &state.instance_name,
                Style::new().add_modifier(Modifier::BOLD),
            ),
        ])),
        header_area,
    );

    let progress = &state.progress;
    let ratio = progress_ratio(progress);
    frame.render_widget(
        Gauge::default()
            .ratio(ratio)
            .label(format!("{:.0}%", ratio * 100.0))
            .gauge_style(Style::new().fg(theme.accent)),
        gauge_area,
    );

    let mut stats = t!(
        "downloads.stats",
        finished = progress.tasks_finished(),
        accepted = progress.tasks_accepted,
        ready = format_bytes(progress.ready_bytes()),
        downloaded = format_bytes(progress.bytes_downloaded),
        speed = format_bytes_per_second(progress.bytes_per_second),
    )
    .into_owned();
    if let Some(eta) = progress.eta {
        write!(stats, "{}", t!("downloads.eta", eta = format_duration(eta)))
            .expect("writing to a String cannot fail");
    }
    if progress.tasks_failed > 0 {
        write!(
            stats,
            "{}",
            t!("downloads.failed_tasks", count = progress.tasks_failed)
        )
        .expect("writing to a String cannot fail");
    }

    frame.render_widget(Paragraph::new(stats), stats_area);
    frame.render_widget(
        Paragraph::new(active_tasks(progress)).wrap(Wrap { trim: true }),
        tasks_area,
    );
}

fn render_message(frame: &mut Frame, area: Rect, message: &str) {
    let paragraph = Paragraph::new(message)
        .style(Style::new().add_modifier(Modifier::BOLD))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn progress_ratio(progress: &DownloadProgress) -> f64 {
    match progress.expected_bytes {
        Some(expected) if expected > 0 => {
            (progress.ready_bytes() as f64 / expected as f64).min(1.0)
        }
        _ if progress.tasks_accepted > 0 => {
            (progress.tasks_finished() as f64 / progress.tasks_accepted as f64).min(1.0)
        }
        _ => 0.0,
    }
}

fn active_tasks(progress: &DownloadProgress) -> String {
    if progress.active_tasks.is_empty() {
        return t!("downloads.waiting").into_owned();
    }

    const VISIBLE: usize = 12;
    let mut lines = progress
        .active_tasks
        .iter()
        .take(VISIBLE)
        .map(|task| {
            let name = task
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| task.path.display().to_string());
            let group = task
                .group
                .clone()
                .unwrap_or_else(|| t!("downloads.group_download").into_owned());
            format!("{group}: {name}")
        })
        .collect::<Vec<String>>();

    let hidden = progress.active_tasks.len().saturating_sub(VISIBLE);
    if hidden > 0 {
        lines.push(t!("downloads.more", count = hidden).into_owned());
    }

    lines.join("\n")
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn format_bytes_per_second(bytes: u64) -> String {
    format!("{}/s", format_bytes(bytes))
}

fn format_duration(duration: std::time::Duration) -> String {
    let seconds = duration.as_secs();
    if seconds >= 3600 {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    } else if seconds >= 60 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}
