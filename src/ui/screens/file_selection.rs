use crate::core::product::{ProductDetector, ProductType};
use crate::ui::components::file_browser::FileBrowser;
use crate::ui::theme::Theme;
use crate::ui::{Screen, ScreenEvent};
use crate::utils::fs::FileSystem;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::path::PathBuf;

pub struct FileSelectionScreen {
    browser: FileBrowser,
    selected_file: Option<PathBuf>,
    detected_product: ProductType,
}

impl FileSelectionScreen {
    pub fn new() -> Self {
        let start_path = FileSystem::get_downloads_dir();
        let detected = ProductDetector::detect_from_directory(&start_path);
        Self {
            browser: FileBrowser::new(start_path),
            selected_file: None,
            detected_product: detected,
        }
    }

    fn refresh_detection(&mut self) {
        let current_dir = self.browser.current_directory();
        self.detected_product = ProductDetector::detect_from_directory(current_dir);
    }
}

impl Screen for FileSelectionScreen {
    fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        let block = Block::default()
            .title(" Select Installer ")
            .borders(Borders::ALL)
            .border_style(Theme::border_style());

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

        let product_info = self.detected_product.to_info();
        let product_style = if self.detected_product == ProductType::Generic {
            Theme::warning_style()
        } else {
            Style::default().fg(Theme::success())
        };

        let instructions_text = if self.detected_product == ProductType::Generic {
            Line::from(vec![
                Span::raw("Select "),
                Span::styled(".exe", Theme::accent_style()),
                Span::raw(" - "),
                Span::styled("Generic application", product_style),
                Span::raw(" (prefix will be created)"),
            ])
        } else {
            Line::from(vec![
                Span::raw("Select "),
                Span::styled("setup.exe", Theme::accent_style()),
                Span::raw(" - Detected: "),
                Span::styled(product_info.display_name, product_style),
            ])
        };

        let instructions = Paragraph::new(instructions_text);
        frame.render_widget(instructions, inner[0]);

        self.browser.render(frame, inner[1]);

        let info_text = if let Some(file) = &self.selected_file {
            format!("Selected: {}", file.display())
        } else {
            "No file selected".to_string()
        };

        let info = Paragraph::new(info_text).style(if self.selected_file.is_some() {
            Theme::success_style()
        } else {
            Theme::normal_style()
        });
        frame.render_widget(info, inner[2]);
    }

    fn handle_input(&mut self, key: KeyEvent) -> Option<ScreenEvent> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => Some(ScreenEvent::Cancel),
            KeyCode::Up => {
                self.browser.move_up();
                None
            }
            KeyCode::Down => {
                self.browser.move_down();
                None
            }
            KeyCode::Left => {
                self.browser.go_to_parent();
                self.refresh_detection();
                None
            }
            KeyCode::Right | KeyCode::Enter => {
                if let Some(selected) = self.browser.get_selected_file() {
                    self.selected_file = Some(selected.clone());
                    Some(ScreenEvent::SelectWithProduct {
                        path: selected.clone(),
                        product: self.detected_product,
                    })
                } else {
                    self.browser.enter_directory();
                    self.refresh_detection();
                    if let Some(file) = self.browser.select_current() {
                        self.selected_file = Some(file.clone());
                        Some(ScreenEvent::SelectWithProduct {
                            path: file,
                            product: self.detected_product,
                        })
                    } else {
                        None
                    }
                }
            }
            KeyCode::Char(' ') => {
                if let Some(file) = self.browser.select_current() {
                    self.selected_file = Some(file);
                }
                None
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.refresh_detection();
                None
            }
            _ => None,
        }
    }
}
