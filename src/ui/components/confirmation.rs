use crate::ui::theme::Theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub struct ConfirmationDialog {
    title: String,
    message: String,
    yes_text: String,
    no_text: String,
    selected: bool, // true = yes, false = no
}

impl ConfirmationDialog {
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            yes_text: "Yes".to_string(),
            no_text: "No".to_string(),
            selected: false,
        }
    }

    pub fn toggle(&mut self) {
        self.selected = !self.selected;
    }

    pub fn is_yes_selected(&self) -> bool {
        self.selected
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let popup_area = Self::centered_rect(60, 40, area);

        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(self.title.clone())
            .borders(Borders::ALL)
            .border_style(Theme::border_style());

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)])
            .split(inner);

        // Mensaje
        let message = Paragraph::new(self.message.clone())
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center);
        frame.render_widget(message, chunks[0]);

        // Botones
        let button_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);

        let yes_style = if self.selected {
            Theme::selected_style()
        } else {
            Theme::normal_style()
        };

        let no_style = if !self.selected {
            Theme::selected_style()
        } else {
            Theme::normal_style()
        };

        let yes_button = Paragraph::new(self.yes_text.clone())
            .style(yes_style)
            .alignment(Alignment::Center);
        let no_button = Paragraph::new(self.no_text.clone())
            .style(no_style)
            .alignment(Alignment::Center);

        frame.render_widget(yes_button, button_layout[0]);
        frame.render_widget(no_button, button_layout[1]);
    }

    fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }
}
