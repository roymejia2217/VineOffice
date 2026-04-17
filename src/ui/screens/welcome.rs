use crate::ui::theme::Theme;
use crate::ui::{Screen, ScreenEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crossterm::event::{KeyCode, KeyEvent};

pub struct WelcomeScreen {
    can_resume: bool,
}

impl WelcomeScreen {
    pub fn new(can_resume: bool) -> Self {
        Self { can_resume }
    }
}

impl Screen for WelcomeScreen {
    fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        let block = Block::default()
            .title(" VineOffice ")
            .borders(Borders::ALL)
            .border_style(Theme::border_style());

        frame.render_widget(block, area);

        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),
                Constraint::Min(10),
                Constraint::Length(3),
            ])
            .margin(2)
            .split(area);

        // ASCII Art Banner
        let ascii_art = vec![
            Line::from("   _   ___          ____  _______        "),
            Line::from("  | | / (_)__  ___ / __ \\/ _/ _(_)______ "),
            Line::from("  | |/ / / _ \\/ -_) /_/ / _/ _/ / __/ -_)"),
            Line::from("  |___/_/_//_/\\__/\\____/_//_//_/\\__/\\__/ "),
        ];
        let banner = Paragraph::new(Text::from(ascii_art))
            .style(Theme::accent_style())
            .alignment(Alignment::Center);
        frame.render_widget(banner, inner[0]);

        // Content
        let content_text = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("Installs Microsoft Office, Project, and Visio on Wine"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Features:", Theme::accent_style()),
            ]),
            Line::from("  - Auto-detection of product type"),
            Line::from("  - Dependency verification"),
            Line::from("  - Step-by-step installation"),
            Line::from("  - Instance management"),
            Line::from("  - Automatic rollback on error"),
            Line::from(""),
            Line::from(vec![
                Span::styled("Requirements:", Theme::accent_style()),
            ]),
            Line::from("  - Wine 10+"),
            Line::from("  - winetricks"),
            Line::from("  - MSI VL setup.exe file"),
        ];

        let content = Paragraph::new(Text::from(content_text))
            .alignment(Alignment::Left);
        frame.render_widget(content, inner[1]);

        // Instructions
        let instructions_text = if self.can_resume {
            "R: Resume  |  N: New  |  I: Instances  |  Q: Quit"
        } else {
            "Enter/N: New  |  I: Instances  |  Q: Quit"
        };
        
        let instructions = Paragraph::new(instructions_text)
            .style(Theme::warning_style())
            .alignment(Alignment::Center);
        frame.render_widget(instructions, inner[2]);
    }

    fn handle_input(&mut self, key: KeyEvent) -> Option<ScreenEvent> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => Some(ScreenEvent::Exit),
            KeyCode::Enter => Some(ScreenEvent::Next),
            KeyCode::Char('n') | KeyCode::Char('N') => Some(ScreenEvent::Next),
            KeyCode::Char('r') | KeyCode::Char('R') if self.can_resume => {
                Some(ScreenEvent::Retry)
            }
            KeyCode::Char('i') | KeyCode::Char('I') => Some(ScreenEvent::ViewInstances),
            _ => None,
        }
    }
}

