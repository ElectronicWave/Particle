use super::{DownloadsContent, detail};
use crate::services::{InstallPhase, InstallSession, Services};
use crate::t;
use crate::theme::Theme;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph, Wrap},
};

pub(super) fn render(
    content: &mut DownloadsContent,
    ctx: &Services,
    frame: &mut Frame,
    area: Rect,
) {
    let theme = ctx.ui().theme();
    let sessions = ctx.install().sessions();

    let [title_area, body_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(area);

    let mut title = vec![Span::styled(
        t!("downloads.title"),
        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
    )];
    if !sessions.is_empty() {
        title.push(Span::styled(
            format!("  {}", sessions.len()),
            Style::new().fg(theme.text_dim),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(title)), title_area);

    if sessions.is_empty() {
        content.list.clear_area();
        render_message(frame, body_area, t!("downloads.empty").as_ref());
        return;
    }

    render_sessions(content, &sessions, frame, body_area, theme);
}

fn render_sessions(
    content: &mut DownloadsContent,
    sessions: &[InstallSession],
    frame: &mut Frame,
    area: Rect,
    theme: Theme,
) {
    let selected = content.list.index().min(sessions.len() - 1);

    if area.width >= 72 {
        let [list_area, _gap, detail_area] = Layout::horizontal([
            Constraint::Length(26),
            Constraint::Length(2),
            Constraint::Fill(1),
        ])
        .areas(area);
        content.list.set_area(list_area, sessions.len());
        render_session_list(sessions, frame, list_area, theme, content.list.state_mut());
        detail::render(&sessions[selected], frame, detail_area, theme);
    } else {
        let list_height = sessions.len().clamp(1, 6) as u16;
        let [list_area, _gap, detail_area] = Layout::vertical([
            Constraint::Length(list_height),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(area);
        content.list.set_area(list_area, sessions.len());
        render_session_list(sessions, frame, list_area, theme, content.list.state_mut());
        detail::render(&sessions[selected], frame, detail_area, theme);
    }
}

fn render_session_list(
    sessions: &[InstallSession],
    frame: &mut Frame,
    area: Rect,
    theme: Theme,
    state: &mut ListState,
) {
    let items = sessions
        .iter()
        .map(|session| {
            let (summary, color) = match &session.phase {
                InstallPhase::Preparing => (
                    format!("{:.0}%", detail::progress_ratio(&session.progress) * 100.0),
                    theme.text_dim,
                ),
                InstallPhase::Done => (t!("downloads.status_done").into_owned(), theme.success),
                InstallPhase::Failed(_) => {
                    (t!("downloads.status_failed").into_owned(), theme.danger)
                }
            };
            let line = Line::from(vec![
                Span::styled(
                    &session.instance_name,
                    Style::new().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(summary, Style::new().fg(color)),
            ]);
            ListItem::new(line)
        })
        .collect::<Vec<ListItem>>();
    let list = List::new(items).highlight_style(theme.selection_style());

    frame.render_stateful_widget(list, area, state);
}

fn render_message(frame: &mut Frame, area: Rect, message: &str) {
    let paragraph = Paragraph::new(message)
        .style(Style::new().add_modifier(Modifier::BOLD))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}
