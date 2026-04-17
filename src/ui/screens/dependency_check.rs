use crate::core::dependencies::{DependencyCheckResult, SystemDependencies};
use crate::ui::theme::Theme;
use crate::ui::{Screen, ScreenEvent};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub struct DependencyCheckScreen {
    result: DependencyCheckResult,
}

impl DependencyCheckScreen {
    pub fn new() -> Self {
        let result = SystemDependencies::verify_all();
        Self { result }
    }
}

impl Screen for DependencyCheckScreen {
    fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        let block = Block::default()
            .title(" Dependencies ")
            .borders(Borders::ALL)
            .border_style(Theme::border_style());

        frame.render_widget(block, area);

        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(5),
            ])
            .margin(2)
            .split(area);

        // General status
        let status = if self.result.all_present {
            Paragraph::new("All dependencies installed")
                .style(Theme::success_style())
                .alignment(Alignment::Center)
        } else {
            Paragraph::new("Missing dependencies")
                .style(Theme::error_style())
                .alignment(Alignment::Center)
        };
        frame.render_widget(status, inner[0]);

        // Dependency list
        let mut lines = vec![
            Line::from(vec![Span::styled(
                "Verified dependencies:",
                Theme::title_style(),
            )]),
            Line::from(""),
        ];

        let deps = vec![
            (
                "wine",
                "Windows API implementation",
                self.result.wine_version.as_deref(),
            ),
            ("winetricks", "Component installer", None),
            ("cabextract", "Archive extractor", None),
            ("winbind", "Authentication service", None),
            (
                "wine32-support",
                "32-bit Wine support (required)",
                None,
            ),
        ];

        let missing: std::collections::HashSet<_> = self.result.missing.iter().copied().collect();

        for (name, desc, version) in deps {
            let is_present = !missing.contains(name);
            let icon = if is_present { "✓" } else { "✗" };
            let color = if is_present {
                Theme::success()
            } else {
                Theme::error()
            };

            let version_str = if let Some(v) = version {
                format!(" ({})", v)
            } else {
                String::new()
            };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} ", icon),
                    ratatui::style::Style::default().fg(color),
                ),
                Span::raw(format!("{}: {}{}", name, desc, version_str)),
            ]));
        }

        lines.push(Line::from(""));

        if !self.result.all_present {
            lines.push(Line::from(vec![Span::styled(
                "Install missing dependencies:",
                Theme::error_style(),
            )]));
            lines.push(Line::from(""));

            for dep in &missing {
                let instruction = SystemDependencies::get_install_instructions(dep);
                lines.push(Line::from(vec![
                    Span::styled(format!("  {}: ", dep), Theme::warning_style()),
                    Span::raw(instruction),
                ]));
            }
        }

        let content = Paragraph::new(lines);
        frame.render_widget(content, inner[1]);

        // Instructions
        let instructions = if self.result.all_present {
            Paragraph::new("Enter: Continue  |  Q: Back")
                .style(Theme::warning_style())
                .alignment(Alignment::Center)
        } else {
            Paragraph::new("Install missing dependencies and restart  |  Q: Back")
                .style(Theme::error_style())
                .alignment(Alignment::Center)
        };
        frame.render_widget(instructions, inner[2]);
    }

    fn handle_input(&mut self, key: KeyEvent) -> Option<ScreenEvent> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => Some(ScreenEvent::Previous),
            KeyCode::Enter => {
                if self.result.all_present {
                    Some(ScreenEvent::Next)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
