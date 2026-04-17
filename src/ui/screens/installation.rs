use crate::core::installation::InstallationManager;
use crate::core::product::{DetectedProduct, ProductDetector, ProductType};
use crate::core::state::{InstallStep, InstallationState, ProgressEvent, StateManager};
use crate::core::wine_prefix::WinePrefixManager;
use crate::ui::components::progress_bar::{StepsProgress, SubProgress};
use crate::ui::components::status_panel::StatusPanel;
use crate::ui::theme::Theme;
use crate::ui::{Screen, ScreenEvent};
use crate::utils::logging::{LogEntry, LogLevel};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crossterm::event::{KeyCode, KeyEvent};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

fn detect_product_from_setup_path(setup_path: &Path, product: ProductType) -> DetectedProduct {
    if let Some(parent) = setup_path.parent() {
        ProductDetector::detect_product_and_version(parent)
    } else {
        DetectedProduct {
            product_type: product,
            version_year: if product.is_office_family() { 2016 } else { 0 },
            edition: if product.is_office_family() { "Standard".to_string() } else { "Generic".to_string() },
        }
    }
}

pub struct InstallationScreen {
    state: InstallationState,
    status_panel: StatusPanel,
    current_message: String,
    is_running: bool,
    is_complete: bool,
    error: Option<String>,
    tx: mpsc::Sender<ProgressEvent>,
    rx: mpsc::Receiver<ProgressEvent>,
    sub_progress: Option<SubProgress>,
    install_handle: Option<JoinHandle<()>>,
}

impl InstallationScreen {
    pub fn new(setup_path: PathBuf, prefix: WinePrefixManager, product: ProductType) -> Self {
        let detected_product = detect_product_from_setup_path(&setup_path, product);

        let state = InstallationState::new(
            setup_path.clone(),
            prefix.get_prefix_path().to_path_buf(),
            product,
        );
        let (tx, rx) = mpsc::channel(100);

        Self {
            state,
            status_panel: StatusPanel::new(),
            current_message: format!("Starting installation of {}...", detected_product.get_display_name()),
            is_running: false,
            is_complete: false,
            error: None,
            tx,
            rx,
            sub_progress: None,
            install_handle: None,
        }
    }

    pub fn from_existing_state(state: InstallationState) -> Self {
        let setup_path = state.setup_path.clone();
        let product = state.get_product_type();
        let (tx, rx) = mpsc::channel(100);

        let detected_product = detect_product_from_setup_path(&setup_path, product);

        Self {
            state,
            status_panel: StatusPanel::new(),
            current_message: format!("Resuming installation of {}...", detected_product.get_display_name()),
            is_running: false,
            is_complete: false,
            error: None,
            tx,
            rx,
            sub_progress: None,
            install_handle: None,
        }
    }

    pub fn start_installation(&mut self) {
        if self.is_running {
            return;
        }

        self.is_running = true;
        let tx = self.tx.clone();
        let product = self.state.get_product_type();
        let manager = InstallationManager::new(
            WinePrefixManager::new(self.state.prefix_path.clone(), crate::core::wine_prefix::WINE_DEFAULT_ARCH),
            self.state.setup_path.clone(),
            product,
        );
        let mut state = self.state.clone();

        let handle = tokio::spawn(async move {
            let steps = InstallStep::all_steps();

            for step in steps {
                if state.completed_steps.contains(&step) {
                    let _ = tx.send(ProgressEvent::Log(
                        LogLevel::Info,
                        format!("{} complete, skipping", step.display_name()),
                    )).await;
                    continue;
                }

                state.set_current_step(step.clone());
                let _ = StateManager::save(&state, &state.prefix_path);

                let _ = tx.send(ProgressEvent::StepStarted(step.clone())).await;

                let result = manager
                    .execute_step(&step, |event| {
                        let _ = tx.try_send(event);
                    })
                    .await;

                match result {
                    Ok(_output) => {
                        state.mark_step_complete(step.clone());
                        let _ = StateManager::save(&state, &state.prefix_path);
                        let _ = tx.send(ProgressEvent::StepCompleted(step.clone())).await;
                    }
                    Err(e) => {
                        state.mark_failed(e.to_string());
                        let _ = StateManager::save(&state, &state.prefix_path);
                        let _ = tx.send(ProgressEvent::Error(e.to_string())).await;
                        break;
                    }
                }
            }

            if state.is_complete() {
                let _ = StateManager::clear(&state.prefix_path);
            }

            let _ = tx.send(ProgressEvent::Completed).await;
        });

        self.install_handle = Some(handle);
    }

    pub fn update(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                ProgressEvent::StepStarted(step) => {
                    self.state.current_step = step.clone();
                    self.sub_progress = None;
                    self.current_message = format!("{}...", step.display_name());
                }
                ProgressEvent::StepCompleted(step) => {
                    self.state.mark_step_complete(step.clone());
                    self.sub_progress = None;
                    self.current_message = format!("{} completed", step.display_name());
                    if let Some(next) = InstallStep::all_steps().get(self.state.completed_steps.len()) {
                        self.state.current_step = next.clone();
                    }
                }
                ProgressEvent::SubProgress { current, total, detail } => {
                    self.sub_progress = Some(SubProgress {
                        current,
                        total,
                        detail: detail.clone(),
                    });
                    self.current_message = detail.clone();
                    self.status_panel.add_log(LogEntry::new(
                        LogLevel::Info,
                        detail,
                    ));
                }
                ProgressEvent::Log(level, msg) => {
                    self.current_message = msg.clone();
                    self.status_panel.add_log(LogEntry::new(level, msg));
                }
                ProgressEvent::Error(msg) => {
                    self.state.failed = true;
                    self.state.error_message = Some(msg.clone());
                    self.error = Some(msg.clone());
                    self.current_message = msg;
                }
                ProgressEvent::Completed => {
                    self.is_running = false;
                    self.is_complete = true;
                    if !self.state.failed {
                        self.sub_progress = None;
                        self.current_message = "Installation completed successfully".to_string();
                    }
                }
            }
        }
    }

    pub fn is_complete(&self) -> bool {
        self.is_complete && !self.state.failed
    }

    pub fn has_error(&self) -> bool {
        self.state.failed
    }

    pub fn get_error(&self) -> Option<String> {
        self.state.error_message.clone()
    }

    pub fn get_prefix_path(&self) -> PathBuf {
        self.state.prefix_path.clone()
    }

    pub fn get_product_type(&self) -> ProductType {
        self.state.get_product_type()
    }
}

impl Screen for InstallationScreen {
    fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        let title = " Installing ";

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Theme::border_style());

        frame.render_widget(block, area);

        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(3),
            ])
            .margin(2)
            .split(area);

        let completed_steps = self.state.completed_steps.len();
        let total_steps = InstallStep::step_count();
        let step_name = self.state.current_step.display_name();
        let has_failed = self.state.failed;

        let progress = StepsProgress::new(
            completed_steps,
            total_steps,
            step_name,
            self.sub_progress.clone(),
        );
        frame.render_widget(progress, inner[0]);

        let message = Paragraph::new(self.current_message.clone())
            .style(if has_failed {
                Theme::error_style()
            } else if self.is_complete {
                Theme::success_style()
            } else {
                Theme::normal_style()
            });
        frame.render_widget(message, inner[1]);

        self.status_panel.render(frame, inner[2]);

        let instructions = if self.is_running {
            Paragraph::new("Installing...").style(Theme::warning_style())
        } else if self.is_complete {
            if has_failed {
                Paragraph::new("R: Retry | Q: Quit").style(Theme::error_style())
            } else {
                Paragraph::new("Enter: Finish | Q: Quit").style(Theme::success_style())
            }
        } else {
            Paragraph::new("Enter: Start installation | Q: Cancel")
                .style(Theme::warning_style())
        };
        frame.render_widget(instructions, inner[3]);
    }

    fn handle_input(&mut self, key: KeyEvent) -> Option<ScreenEvent> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => Some(ScreenEvent::Cancel),
            KeyCode::Enter => {
                if !self.is_running {
                    if self.is_complete && !self.state.failed {
                        Some(ScreenEvent::Complete)
                    } else if self.state.failed {
                        // Retry
                        self.state.failed = false;
                        self.state.error_message = None;
                        self.is_complete = false;
                        self.start_installation();
                        None
                    } else {
                        self.start_installation();
                        None
                    }
                } else {
                    None
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if self.state.failed && !self.is_running {
                    self.state.failed = false;
                    self.state.error_message = None;
                    self.is_complete = false;
                    self.start_installation();
                }
                None
            }
            KeyCode::Up => {
                self.status_panel.scroll_up();
                None
            }
            KeyCode::Down => {
                self.status_panel.scroll_down();
                None
            }
            _ => None,
        }
    }
}
