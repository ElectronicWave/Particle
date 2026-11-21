use std::fmt::Write;

use crate::services::{InstallPhase, InstallSession};
use crate::t;
use crate::theme::Theme;
use elemental::infra::downloader::DownloadProgress;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph, Wrap},
};

pub(super) fn render(session: &InstallSession, frame: &mut Frame, area: Rect, theme: Theme) {
    match &session.phase {
        InstallPhase::Preparing => render_progress(frame, area, session, theme),
        InstallPhase::Done => render_done(frame, area, &session.instance_name, theme),
        InstallPhase::Failed(error) => render_failed(frame, area, error, theme),
    }
}

fn render_done(frame: &mut Frame, area: Rect, instance_name: &str, theme: Theme) {
    let lines = vec![
        Line::styled(
            t!("downloads.status_done"),
            Style::new().fg(theme.success).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::styled(
            t!("downloads.done", instance = instance_name),
            Style::new().fg(theme.text_dim),
        ),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_failed(frame: &mut Frame, area: Rect, error: &str, theme: Theme) {
    let lines = vec![
        Line::styled(
            t!("downloads.failed"),
            Style::new().fg(theme.danger).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::raw(error.to_owned()),
        Line::raw(""),
        Line::styled(t!("downloads.dismiss"), Style::new().fg(theme.text_dim)),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_progress(frame: &mut Frame, area: Rect, session: &InstallSession, theme: Theme) {
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
                &session.instance_name,
                Style::new().add_modifier(Modifier::BOLD),
            ),
        ])),
        header_area,
    );

    let progress = &session.progress;
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

pub(super) fn progress_ratio(progress: &DownloadProgress) -> f64 {
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
