use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditEvent {
    Unhandled,
    Changed,
    Submit,
    Cancel,
}

/// Line editor state shared by settings panes and future input widgets.
#[derive(Debug, Clone)]
pub struct TextEdit {
    input: String,
}

impl TextEdit {
    pub fn new(input: String) -> Self {
        Self { input }
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn cursor_display(&self) -> String {
        format!("{}█", self.input)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> EditEvent {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return EditEvent::Unhandled;
        }

        match key.code {
            KeyCode::Esc => EditEvent::Cancel,
            KeyCode::Backspace => {
                self.input.pop();
                EditEvent::Changed
            }
            KeyCode::Enter => EditEvent::Submit,
            KeyCode::Char(character) => {
                self.input.push(character);
                EditEvent::Changed
            }
            _ => EditEvent::Unhandled,
        }
    }
}
