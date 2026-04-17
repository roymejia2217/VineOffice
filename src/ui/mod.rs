pub mod components;
pub mod screens;
pub mod theme;

use crate::core::product::ProductType;
use ratatui::Frame;
use std::path::PathBuf;

pub trait Screen {
    fn render(&self, frame: &mut Frame);
    fn handle_input(&mut self, key: crossterm::event::KeyEvent) -> Option<ScreenEvent>;
}

#[derive(Clone, Debug)]
pub enum ScreenEvent {
    Next,
    Previous,
    Cancel,
    Complete,
    SelectWithProduct { path: PathBuf, product: ProductType },
    Retry,
    Exit,
    ViewInstances,
    LaunchInstance(PathBuf, ProductType),
    DeleteInstance(PathBuf),
    RepairDesktopIntegration(PathBuf),
}
