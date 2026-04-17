use crate::core::desktop_integration::DesktopIntegration;
use crate::core::instance_manager::InstanceManager;
use crate::core::product::{DetectedProduct, ProductDetector, ProductType};
use crate::core::state::{InstallationState, StateManager};
use crate::core::wine_prefix::WinePrefixManager;
use crate::ui::components::confirmation::ConfirmationDialog;
use crate::ui::screens::completion::CompletionScreen;
use crate::ui::screens::dependency_check::DependencyCheckScreen;
use crate::ui::screens::file_selection::FileSelectionScreen;
use crate::ui::screens::installation::InstallationScreen;
use crate::ui::screens::instance_list::InstanceListScreen;
use crate::ui::screens::welcome::WelcomeScreen;
use crate::ui::{Screen, ScreenEvent};
use ratatui::{backend::Backend, Terminal};
use tracing::info;

pub enum AppState {
    Welcome(WelcomeScreen),
    InstanceList(InstanceListScreen),
    DependencyCheck(DependencyCheckScreen),
    FileSelection(FileSelectionScreen),
    Installation(InstallationScreen),
    Completion(CompletionScreen),
}

pub struct App {
    state: AppState,
    should_exit: bool,
    resume_state: Option<InstallationState>,
    confirm_dialog: Option<ConfirmationDialog>,
}

impl App {
    pub fn new() -> Self {
        let can_resume = StateManager::can_resume_any();
        info!("Starting VineOffice. Resumable installation: {}", can_resume);

        Self {
            state: AppState::Welcome(WelcomeScreen::new(can_resume)),
            should_exit: false,
            resume_state: None,
            confirm_dialog: None,
        }
    }

    pub fn run<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> anyhow::Result<()> {
        while !self.should_exit {
            terminal.draw(|frame| {
                match &self.state {
                    AppState::Welcome(s) => s.render(frame),
                    AppState::InstanceList(s) => s.render(frame),
                    AppState::DependencyCheck(s) => s.render(frame),
                    AppState::FileSelection(s) => s.render(frame),
                    AppState::Installation(s) => s.render(frame),
                    AppState::Completion(s) => s.render(frame),
                }
                if let Some(dialog) = &self.confirm_dialog {
                    dialog.render(frame);
                }
            })?;

            if crossterm::event::poll(std::time::Duration::from_millis(50))? {
                if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                    self.handle_input(key);
                }
            }

            if let AppState::Installation(screen) = &mut self.state {
                screen.update();

                if screen.is_complete() {
                    let prefix = WinePrefixManager::new(
                        screen.get_prefix_path().to_path_buf(),
                        crate::core::wine_prefix::WINE_DEFAULT_ARCH
                    );
                    let product = screen.get_product_type();
                    self.state = AppState::Completion(CompletionScreen::success(prefix, product));
                } else if screen.has_error() {
                    let error = screen.get_error().unwrap_or_else(|| "Unknown error".to_string());
                    self.state = AppState::Completion(CompletionScreen::error(error));
                }
            }

            if let AppState::InstanceList(screen) = &mut self.state {
                screen.check_pending_delete();
                screen.check_pending_repair();
                screen.check_loading();
            }
        }

        Ok(())
    }

    fn handle_input(&mut self, key: crossterm::event::KeyEvent) {
        if let Some(dialog) = &mut self.confirm_dialog {
            match key.code {
                crossterm::event::KeyCode::Left | crossterm::event::KeyCode::Right => {
                    dialog.toggle();
                }
                crossterm::event::KeyCode::Enter => {
                    if dialog.is_yes_selected() {
                        self.confirm_dialog = None;
                        if let Some(state) = self.resume_state.take() {
                            let screen = InstallationScreen::from_existing_state(state);
                            self.state = AppState::Installation(screen);
                        }
                    } else {
                        self.confirm_dialog = None;
                    }
                }
                crossterm::event::KeyCode::Esc => {
                    self.confirm_dialog = None;
                }
                _ => {}
            }
            return;
        }

        let event = match &mut self.state {
            AppState::Welcome(screen) => screen.handle_input(key),
            AppState::InstanceList(screen) => screen.handle_input(key),
            AppState::DependencyCheck(screen) => screen.handle_input(key),
            AppState::FileSelection(screen) => screen.handle_input(key),
            AppState::Installation(screen) => screen.handle_input(key),
            AppState::Completion(screen) => screen.handle_input(key),
        };

        if let Some(event) = event {
            self.handle_screen_event(event);
        }
    }

    fn handle_screen_event(&mut self, event: ScreenEvent) {
        match event {
            ScreenEvent::Next => match &self.state {
                AppState::Welcome(_) => {
                    self.state = AppState::DependencyCheck(DependencyCheckScreen::new());
                }
                AppState::InstanceList(_) => {
                    self.state = AppState::DependencyCheck(DependencyCheckScreen::new());
                }
                AppState::DependencyCheck(_) => {
                    self.state = AppState::FileSelection(FileSelectionScreen::new());
                }
                AppState::FileSelection(_) => {}
                AppState::Installation(_) => {}
                AppState::Completion(_) => {
                    self.should_exit = true;
                }
            },
            ScreenEvent::Previous => match &self.state {
                AppState::InstanceList(_) => {
                    let can_resume = StateManager::can_resume_any();
                    self.state = AppState::Welcome(WelcomeScreen::new(can_resume));
                }
                AppState::DependencyCheck(_) => {
                    let can_resume = StateManager::can_resume_any();
                    self.state = AppState::Welcome(WelcomeScreen::new(can_resume));
                }
                AppState::FileSelection(_) => {
                    self.state = AppState::DependencyCheck(DependencyCheckScreen::new());
                }
                _ => {}
            },
            ScreenEvent::Cancel => match &self.state {
                AppState::FileSelection(_) => {
                    self.state = AppState::DependencyCheck(DependencyCheckScreen::new());
                }
                AppState::Installation(_) => {
                    self.confirm_dialog = Some(ConfirmationDialog::new(
                        "Cancel Installation",
                        "Cancel installation? This will trigger an automatic rollback."
                    ));
                }
                _ => {}
            },
            ScreenEvent::Complete => {
                let (prefix, product) = match &self.state {
                    AppState::Installation(screen) => {
                        let prefix = WinePrefixManager::new(
                            screen.get_prefix_path().to_path_buf(),
                            crate::core::wine_prefix::WINE_DEFAULT_ARCH
                        );
                        let product = screen.get_product_type();
                        (prefix, product)
                    }
                    _ => (WinePrefixManager::default_office_prefix(), ProductType::Generic),
                };
                self.state = AppState::Completion(CompletionScreen::success(prefix, product));
            }
            ScreenEvent::SelectWithProduct { path, product } => {
                let detected_product = if let Some(parent) = path.parent() {
                    ProductDetector::detect_product_and_version(parent)
                } else {
                    DetectedProduct {
                        product_type: product,
                        version_year: if product.is_office_family() { 2016 } else { 0 },
                        edition: if product.is_office_family() { "Standard".to_string() } else { "Generic".to_string() },
                    }
                };

                let prefix = if detected_product.product_type.is_office_family() {
                    WinePrefixManager::for_product_with_version(
                        detected_product.product_type,
                        detected_product.version_year
                    )
                } else {
                    WinePrefixManager::for_product(ProductType::Generic)
                };

                let screen = InstallationScreen::new(path, prefix, detected_product.product_type);
                self.state = AppState::Installation(screen);
            }
            ScreenEvent::Retry => match &self.state {
                AppState::Welcome(_) => {
                    if let Some(state) = StateManager::load_any_resumable() {
                        self.confirm_dialog = Some(ConfirmationDialog::new(
                            "Resume Installation",
                            &format!(
                                "Resume installation from step {:?}?",
                                state.current_step
                            ),
                        ));
                        self.resume_state = Some(state);
                    }
                }
                _ => {}
            },
            ScreenEvent::Exit => {
                self.should_exit = true;
            }
            ScreenEvent::ViewInstances => {
                self.state = AppState::InstanceList(InstanceListScreen::new());
            }
            ScreenEvent::LaunchInstance(path, product) => {
                tokio::spawn(async move {
                    if let Err(e) = InstanceManager::launch_product(&path, product).await {
                        let product_name = product.to_info().display_name;
                        tracing::error!("Failed to launch {}: {}", product_name, e);
                    }
                });
            }
            ScreenEvent::DeleteInstance(path) => {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                let path_clone = path.clone();
                tokio::spawn(async move {
                    let result = InstanceManager::delete_instance(&path_clone).await;
                    if let Err(e) = &result {
                        tracing::error!("Failed to delete instance: {}", e);
                    }
                    let _ = tx.send(result.is_ok()).await;
                });
                if let AppState::InstanceList(screen) = &mut self.state {
                    screen.set_delete_pending(rx);
                }
            }
            ScreenEvent::RepairDesktopIntegration(path) => {
                let path_clone = path.clone();
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                
                tokio::spawn(async move {
                    let prefix_manager = match WinePrefixManager::from_existing_path(path_clone) {
                        Ok(pm) => pm,
                        Err(e) => {
                            let err_msg = format!("Invalid prefix: {}", e);
                            tracing::error!("{}", err_msg);
                            let _ = tx.send(Err(err_msg)).await;
                            return;
                        }
                    };
                    match DesktopIntegration::create_entries_for_prefix(&prefix_manager).await {
                        Ok(_) => {
                            tracing::info!("Desktop entries regenerated successfully");
                            let _ = tx.send(Ok(())).await;
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to regenerate desktop entries: {}", e);
                            tracing::error!("{}", err_msg);
                            let _ = tx.send(Err(err_msg)).await;
                        }
                    }
                });

                if let AppState::InstanceList(screen) = &mut self.state {
                    screen.set_repair_pending(rx);
                }
            }
        }
    }
}
