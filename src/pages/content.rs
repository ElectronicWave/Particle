use ratail::{SnapshotStatus, SnapshotView};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::Line,
    widgets::{Paragraph, Widget},
};

use crate::services::Services;
use crate::t;
use crate::widgets::spinner;

pub fn render_message(frame: &mut Frame, area: Rect, message: &str) {
    Paragraph::new(message)
        .centered()
        .render(area, frame.buffer_mut());
}

pub fn render_loading(ctx: &Services, frame: &mut Frame, area: Rect, message: &str) {
    ctx.ui().request_frame(spinner::INTERVAL);
    let line = Line::raw(format!("{} {message}", spinner::frame()));
    frame.render_widget(Paragraph::new(line).centered(), area);
}

pub fn render_snapshot<T, F>(
    ctx: &Services,
    view: &SnapshotView<'_, T, anyhow::Error>,
    loading: &str,
    frame: &mut Frame,
    area: Rect,
    render_data: F,
) where
    F: FnOnce(&T, &mut Frame, Rect),
{
    match (view.data, view.error) {
        (Some(data), Some(error)) => {
            let [error_area, body_area] =
                Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);
            render_message(frame, error_area, &t!("common.error", error = error));
            render_data(data, frame, body_area);
        }
        (Some(data), None) => render_data(data, frame, area),
        (None, Some(error)) => render_message(frame, area, &t!("common.error", error = error)),
        (None, None) if view.status == SnapshotStatus::Cancelled => {
            render_message(frame, area, &t!("common.cancelled"))
        }
        (None, None) => render_loading(ctx, frame, area, loading),
    }
}
