use crate::core::instance_manager::{InstanceManager, WineInstance};
use crate::core::product::ProductType;
use crate::ui::components::confirmation::ConfirmationDialog;
use crate::ui::theme::Theme;
use crate::ui::{Screen, ScreenEvent};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};
use std::path::PathBuf;
use tokio::sync::mpsc;

pub struct InstanceListScreen {
    instances: Vec<WineInstance>,
    selected_index: usize,
    confirm_dialog: Option<ConfirmationDialog>,
    dialog_action: Option<DialogAction>,
    message: Option<String>,
    delete_complete_rx: Option<mpsc::Receiver<bool>>,
    repair_complete_rx: Option<mpsc::Receiver<Result<(), String>>>,
    loading_rx: Option<mpsc::Receiver<Vec<WineInstance>>>,
    is_loading: bool,
}

enum DialogAction {
    Delete(PathBuf),
    Launch(PathBuf, ProductType),
    RepairDesktop(PathBuf),
}

impl InstanceListScreen {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(1);
        tokio::spawn(async move {
            let instances = InstanceManager::detect_instances().await;
            let _ = tx.send(instances).await;
        });

        Self {
            instances: Vec::new(),
            selected_index: 0,
            confirm_dialog: None,
            dialog_action: None,
            message: Some("Loading instances...".to_string()),
            delete_complete_rx: None,
            repair_complete_rx: None,
            loading_rx: Some(rx),
            is_loading: true,
        }
    }

    pub fn refresh(&mut self) {
        let (tx, rx) = mpsc::channel(1);
        tokio::spawn(async move {
            let instances = InstanceManager::detect_instances().await;
            let _ = tx.send(instances).await;
        });
        self.loading_rx = Some(rx);
        self.is_loading = true;
        self.message = Some("Refreshing...".to_string());
    }

    /// Called each frame from App::run to receive async-loaded instances
    pub fn check_loading(&mut self) {
        if let Some(rx) = &mut self.loading_rx {
            if let Ok(instances) = rx.try_recv() {
                self.instances = instances;
                self.is_loading = false;
                if self.selected_index >= self.instances.len() && !self.instances.is_empty() {
                    self.selected_index = self.instances.len() - 1;
                }
                self.message = if self.instances.is_empty() {
                    Some("No instances found. Press N to install.".to_string())
                } else {
                    None
                };
                self.loading_rx = None;
            }
        }
    }

    pub fn set_delete_pending(&mut self, rx: mpsc::Receiver<bool>) {
        self.delete_complete_rx = Some(rx);
        self.message = Some("Deleting instance...".to_string());
    }

    pub fn set_repair_pending(&mut self, rx: mpsc::Receiver<Result<(), String>>) {
        self.repair_complete_rx = Some(rx);
        self.message = Some("Repairing desktop integration...".to_string());
    }

    pub fn check_pending_repair(&mut self) {
        if let Some(rx) = &mut self.repair_complete_rx {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(()) => {
                        self.message = Some("Desktop entries regenerated successfully".to_string());
                    }
                    Err(e) => {
                        self.message = Some(format!("Failed to repair: {}", e));
                    }
                }
                self.repair_complete_rx = None;
            }
        }
    }

    pub fn check_pending_delete(&mut self) {
        if let Some(rx) = &mut self.delete_complete_rx {
            if rx.try_recv().is_ok() {
                self.refresh();
                self.message = Some("Instance deleted successfully".to_string());
                self.delete_complete_rx = None;
            }
        }
    }

    fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    fn move_down(&mut self) {
        if self.selected_index + 1 < self.instances.len() {
            self.selected_index += 1;
        }
    }

    fn get_selected_instance(&self) -> Option<&WineInstance> {
        self.instances.get(self.selected_index)
    }

    fn render_instances_table(&self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec![
            Cell::from("Name").style(Theme::title_style()),
            Cell::from("Product").style(Theme::title_style()),
            Cell::from("Size").style(Theme::title_style()),
            Cell::from("Status").style(Theme::title_style()),
        ])
        .height(1)
        .style(Style::default().add_modifier(Modifier::BOLD));

        let rows: Vec<Row> = self
            .instances
            .iter()
            .enumerate()
            .map(|(idx, instance)| {
                let is_selected = idx == self.selected_index;

                let name_style = if is_selected {
                    Theme::accent_style().add_modifier(Modifier::BOLD)
                } else {
                    Theme::normal_style()
                };

                let product_short = match instance.product_type {
                    ProductType::Office2016 => "Office",
                    ProductType::Project2016 => "Project",
                    ProductType::Visio2016 => "Visio",
                    ProductType::Generic => "Generic",
                };

                let status = if instance.is_installed {
                    ("Installed", Theme::success_style())
                } else {
                    ("Not installed", Theme::error_style())
                };

                let cells = vec![
                    Cell::from(instance.name.clone()).style(name_style),
                    Cell::from(product_short).style(Theme::normal_style()),
                    Cell::from(crate::utils::format::human_readable_size(
                        instance.size_bytes,
                    ))
                    .style(Theme::normal_style()),
                    Cell::from(status.0).style(status.1),
                ];

                let row = Row::new(cells).height(1);

                if is_selected {
                    row.style(Style::default().bg(ratatui::style::Color::DarkGray))
                } else {
                    row
                }
            })
            .collect();

        let widths = [
            Constraint::Percentage(30),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(35),
        ];

        let table = Table::new(rows, widths).header(header).block(
            Block::default()
                .title(" Detected Instances ")
                .borders(Borders::ALL)
                .border_style(Theme::border_style()),
        );

        frame.render_widget(table, area);
    }

    fn render_help(&self, frame: &mut Frame, area: Rect) {
        let help_text = if self.instances.is_empty() {
            vec![
                Line::from("No instances found."),
                Line::from(""),
                Line::from("Press N to create a new installation."),
            ]
        } else {
            vec![
                Line::from(vec![
                    Span::styled("Up/Down", Theme::accent_style()),
                    Span::raw(": Navigate  "),
                    Span::styled("Enter/E", Theme::accent_style()),
                    Span::raw(": Launch  "),
                    Span::styled("F", Theme::accent_style()),
                    Span::raw(": Open folder"),
                ]),
                Line::from(vec![
                    Span::styled("D", Theme::accent_style()),
                    Span::raw(": Delete  "),
                    Span::styled("I", Theme::accent_style()),
                    Span::raw(": Repair integration  "),
                    Span::styled("R", Theme::accent_style()),
                    Span::raw(": Refresh"),
                ]),
                Line::from(vec![
                    Span::styled("N", Theme::accent_style()),
                    Span::raw(": New installation  "),
                    Span::styled("Q", Theme::accent_style()),
                    Span::raw(": Back to menu"),
                ]),
            ]
        };

        let help = Paragraph::new(Text::from(help_text))
            .alignment(Alignment::Center)
            .style(Theme::normal_style());

        frame.render_widget(help, area);
    }

    fn render_instance_details(&self, frame: &mut Frame, area: Rect) {
        if let Some(instance) = self.get_selected_instance() {
            let product_name = instance.product_type.to_info().display_name;
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Name: ", Theme::title_style()),
                    Span::raw(&instance.name),
                ]),
                Line::from(vec![
                    Span::styled("Product: ", Theme::title_style()),
                    Span::raw(product_name),
                ]),
                Line::from(vec![
                    Span::styled("Path: ", Theme::title_style()),
                    Span::raw(instance.path.display().to_string()),
                ]),
                Line::from(vec![
                    Span::styled("Size: ", Theme::title_style()),
                    Span::raw(crate::utils::format::human_readable_size(
                        instance.size_bytes,
                    )),
                ]),
            ];

            if let Some(created) = instance.created_at {
                if let Ok(duration) = created.elapsed() {
                    let days = duration.as_secs() / 86400;
                    let time_str = if days == 0 {
                        "Today".to_string()
                    } else if days == 1 {
                        "Yesterday".to_string()
                    } else {
                        format!("{} days ago", days)
                    };
                    lines.push(Line::from(vec![
                        Span::styled("Created: ", Theme::title_style()),
                        Span::raw(time_str),
                    ]));
                }
            }

            let status = if instance.is_installed {
                "Installed"
            } else {
                "Not installed"
            };
            lines.push(Line::from(vec![
                Span::styled("Status: ", Theme::title_style()),
                Span::styled(
                    status,
                    if instance.is_installed {
                        Theme::success_style()
                    } else {
                        Theme::error_style()
                    },
                ),
            ]));

            let details = Paragraph::new(Text::from(lines)).block(
                Block::default()
                    .title(" Details ")
                    .borders(Borders::ALL)
                    .border_style(Theme::border_style()),
            );

            frame.render_widget(details, area);
        }
    }
}

impl Screen for InstanceListScreen {
    fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        let block = Block::default()
            .title(" Instances ")
            .borders(Borders::ALL)
            .border_style(Theme::border_style());

        frame.render_widget(block, area);

        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(8),
                Constraint::Length(8),
                Constraint::Length(4),
            ])
            .margin(2)
            .split(area);

        let title = Paragraph::new("Detected Instances")
            .style(Theme::title_style())
            .alignment(Alignment::Center);
        frame.render_widget(title, inner[0]);

        if !self.instances.is_empty() {
            self.render_instances_table(frame, inner[1]);
            self.render_instance_details(frame, inner[2]);
        } else {
            let empty_msg = Paragraph::new("No instances found")
                .style(Theme::warning_style())
                .alignment(Alignment::Center);
            frame.render_widget(empty_msg, inner[1]);
        }

        self.render_help(frame, inner[3]);

        if let Some(dialog) = &self.confirm_dialog {
            dialog.render(frame);
        }
    }

    fn handle_input(&mut self, key: KeyEvent) -> Option<ScreenEvent> {
        if let Some(dialog) = &mut self.confirm_dialog {
            match key.code {
                KeyCode::Left | KeyCode::Right => {
                    dialog.toggle();
                    return None;
                }
                KeyCode::Enter => {
                    if dialog.is_yes_selected() {
                        if let Some(action) = self.dialog_action.take() {
                            self.confirm_dialog = None;
                            match action {
                                DialogAction::Delete(path) => {
                                    return Some(ScreenEvent::DeleteInstance(path));
                                }
                                DialogAction::Launch(path, product) => {
                                    return Some(ScreenEvent::LaunchInstance(path, product));
                                }
                                DialogAction::RepairDesktop(path) => {
                                    return Some(ScreenEvent::RepairDesktopIntegration(path));
                                }
                            }
                        }
                    } else {
                        self.confirm_dialog = None;
                        self.dialog_action = None;
                    }
                    return None;
                }
                KeyCode::Esc => {
                    self.confirm_dialog = None;
                    self.dialog_action = None;
                    return None;
                }
                _ => return None,
            }
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => Some(ScreenEvent::Previous),
            KeyCode::Char('n') | KeyCode::Char('N') => Some(ScreenEvent::Next),
            KeyCode::Up => {
                self.move_up();
                None
            }
            KeyCode::Down => {
                self.move_down();
                None
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.refresh();
                self.message = Some("List updated".to_string());
                None
            }
            KeyCode::Char('e') | KeyCode::Char('E') | KeyCode::Enter => {
                if let Some(instance) = self.get_selected_instance().cloned() {
                    if instance.is_installed {
                        let product_name = instance.product_type.to_info().display_name;
                        self.dialog_action = Some(DialogAction::Launch(
                            instance.path.clone(),
                            instance.product_type,
                        ));
                        self.confirm_dialog = Some(ConfirmationDialog::new(
                            "Launch Application",
                            &format!("Launch {} from '{}'?", product_name, instance.name),
                        ));
                    } else {
                        self.message = Some("Product not installed in this instance".to_string());
                    }
                }
                None
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if let Some(instance) = self.get_selected_instance().cloned() {
                    self.dialog_action = Some(DialogAction::Delete(instance.path.clone()));
                    self.confirm_dialog = Some(ConfirmationDialog::new(
                        "Delete Instance",
                        &format!(
                            "Delete '{}' permanently?\n{} will be freed.",
                            instance.name,
                            crate::utils::format::human_readable_size(instance.size_bytes)
                        ),
                    ));
                }
                None
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {
                if let Some(instance) = self.get_selected_instance().cloned() {
                    InstanceManager::open_in_file_manager(&instance.path);
                }
                None
            }
            KeyCode::Char('i') | KeyCode::Char('I') => {
                if let Some(instance) = self.get_selected_instance().cloned() {
                    if instance.is_installed {
                        self.dialog_action =
                            Some(DialogAction::RepairDesktop(instance.path.clone()));
                        self.confirm_dialog = Some(ConfirmationDialog::new(
                            "Repair Desktop Integration",
                            &format!(
                                "Regenerate desktop entries and MIME associations for '{}'?\n\nThis will recreate application shortcuts.",
                                instance.name
                            ),
                        ));
                    } else {
                        self.message = Some("Product not installed in this instance".to_string());
                    }
                }
                None
            }
            _ => None,
        }
    }
}
