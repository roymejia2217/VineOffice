use crate::core::product::ProductType;
use crate::core::wine_prefix::WinePrefixManager;
use crate::ui::theme::Theme;
use crate::ui::{Screen, ScreenEvent};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub struct CompletionScreen {
    success: bool,
    error_message: Option<String>,
    prefix: Option<WinePrefixManager>,
    product: ProductType,
}

impl CompletionScreen {
    pub fn success(prefix: WinePrefixManager, product: ProductType) -> Self {
        Self {
            success: true,
            error_message: None,
            prefix: Some(prefix),
            product,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            error_message: Some(message.into()),
            prefix: None,
            product: ProductType::Generic,
        }
    }
}

impl Screen for CompletionScreen {
    fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        let title = if self.success {
            " Installation Complete "
        } else {
            " Installation Failed "
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(if self.success {
                Theme::success_style()
            } else {
                Theme::error_style()
            });

        frame.render_widget(block, area);

        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(3),
            ])
            .margin(2)
            .split(area);

        let product_name = self.product.to_info().display_name;
        let status = if self.success {
            Paragraph::new(format!("{} installed successfully", product_name))
                .style(Theme::success_style())
                .alignment(Alignment::Center)
        } else {
            Paragraph::new("Installation failed")
                .style(Theme::error_style())
                .alignment(Alignment::Center)
        };
        frame.render_widget(status, inner[0]);

        let mut lines = vec![];

        if self.success {
            if let Some(prefix) = &self.prefix {
                lines.push(Line::from(vec![Span::styled(
                    "Installation details:",
                    Theme::title_style(),
                )]));
                lines.push(Line::from(""));

                lines.push(Line::from(vec![
                    Span::raw("WINEPREFIX: "),
                    Span::styled(
                        prefix.get_prefix_path().to_string_lossy().to_string(),
                        Theme::accent_style(),
                    ),
                ]));

                if self.product.is_office_family() {
                    let product_info = self.product.to_info();
                    let exe_name = product_info.exe_name.trim_end_matches(".EXE");
                    let exe_path = prefix.get_product_exe_path(self.product);
                    lines.push(Line::from(vec![
                        Span::raw(format!("{}: ", exe_name)),
                        Span::styled(
                            exe_path.to_string_lossy().to_string(),
                            Theme::accent_style(),
                        ),
                    ]));
                }

                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::raw("Product: "),
                    Span::styled(self.product.as_str(), Theme::accent_style()),
                ]));

                if self.product == ProductType::Generic {
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![
                        Span::styled("Note: ", Theme::warning_style()),
                        Span::raw("Generic application installed."),
                    ]));
                    lines.push(Line::from(vec![
                        Span::raw("  Look for it in the "),
                        Span::styled("Wine", Theme::accent_style()),
                        Span::raw(" category of your application menu."),
                    ]));
                }
            }
        } else if let Some(error) = &self.error_message {
            lines.push(Line::from(vec![Span::styled(
                "Error:",
                Theme::error_style(),
            )]));
            lines.push(Line::from(error.clone()));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "Suggestions:",
                Theme::warning_style(),
            )]));
            lines.push(Line::from("  1. Verify setup.exe integrity"));
            lines.push(Line::from("  2. Check available disk space"));
            lines.push(Line::from(
                "  3. Check logs at ~/.local/share/vineoffice/logs/",
            ));
        }

        let content = Paragraph::new(lines);
        frame.render_widget(content, inner[1]);

        let help_text = if self.success {
            if self.product.is_office_family() {
                let product_info = self.product.to_info();
                let short_name = product_info.display_name;
                format!("E: Launch {} | Enter: Quit | Q: Quit", short_name)
            } else {
                "F: Open prefix folder | Enter: Quit | Q: Quit".to_string()
            }
        } else {
            "Enter: Quit | Q: Quit".to_string()
        };
        let instructions = Paragraph::new(help_text)
            .style(Theme::warning_style())
            .alignment(Alignment::Center);
        frame.render_widget(instructions, inner[2]);
    }

    fn handle_input(&mut self, key: KeyEvent) -> Option<ScreenEvent> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Enter => Some(ScreenEvent::Exit),
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if self.success && self.product.is_office_family() {
                    if let Some(prefix) = &self.prefix {
                        return Some(ScreenEvent::LaunchInstance(
                            prefix.get_prefix_path().to_path_buf(),
                            self.product,
                        ));
                    }
                }
                None
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {
                if self.success && self.product == ProductType::Generic {
                    if let Some(prefix) = &self.prefix {
                        crate::core::instance_manager::InstanceManager::open_in_file_manager(
                            prefix.get_prefix_path(),
                        );
                    }
                }
                None
            }
            _ => None,
        }
    }
}
