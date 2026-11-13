use ratatui::{
    buffer::Buffer,
    layout::{Rect, Size},
    widgets::StatefulWidget,
};
use tui_scrollview::{ScrollView, ScrollViewState, ScrollbarVisibility};

pub struct SelectedScrollView<'a, T, R>
where
    R: Fn(usize, &T, bool, Rect, &mut Buffer),
{
    items: &'a [T],
    renderer: R,
}

impl<'a, T, R> SelectedScrollView<'a, T, R>
where
    R: Fn(usize, &T, bool, Rect, &mut Buffer),
{
    pub fn new(items: &'a [T], renderer: R) -> Self {
        Self { items, renderer }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SelectedScrollViewState {
    pub selected: usize,
    pub scroll: ScrollViewState,
}

impl SelectedScrollViewState {
    pub fn down(&mut self, len: usize) {
        if len == 0 {
            return;
        }

        self.selected = self.selected.saturating_add(1).min(len - 1);
        self.scroll.scroll_down();
    }

    pub fn up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.scroll.scroll_up();
    }
}

impl<'a, T, R> StatefulWidget for SelectedScrollView<'a, T, R>
where
    R: Fn(usize, &T, bool, Rect, &mut Buffer),
{
    type State = SelectedScrollViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let mut view = ScrollView::new(Size::new(area.width, self.items.len() as u16))
            .horizontal_scrollbar_visibility(ScrollbarVisibility::Never);

        for (index, item) in self.items.iter().enumerate() {
            let item_area = Rect::new(0, index as u16, area.width.saturating_sub(1), 1);

            (self.renderer)(
                index,
                item,
                index == state.selected,
                item_area,
                view.buf_mut(),
            );
        }

        view.render(area, buf, &mut state.scroll);
    }
}
