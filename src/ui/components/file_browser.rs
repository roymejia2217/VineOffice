use crate::ui::theme::Theme;
use crate::utils::fs::{DirEntry, FileSystem};
use ratatui::{
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use std::cell::Cell;
use std::path::PathBuf;

pub struct FileBrowser {
    current_path: PathBuf,
    entries: Vec<DirEntry>,
    selected_index: usize,
    selected_file: Option<PathBuf>,
    scroll_offset: usize,
    last_max_visible: Cell<usize>,
}

impl FileBrowser {
    pub fn new(start_path: PathBuf) -> Self {
        let entries = FileSystem::list_directory(&start_path);
        Self {
            current_path: start_path,
            entries,
            selected_index: 0,
            selected_file: None,
            scroll_offset: 0,
            last_max_visible: Cell::new(15), // initial estimate, updated on render
        }
    }

    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.adjust_scroll();
        }
    }

    pub fn move_down(&mut self) {
        if self.selected_index < self.entries.len().saturating_sub(1) {
            self.selected_index += 1;
            self.adjust_scroll();
        }
    }

    pub fn adjust_scroll(&mut self) {
        let max = self.last_max_visible.get();
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + max {
            self.scroll_offset = self.selected_index.saturating_sub(max - 1);
        }
    }

    pub fn enter_directory(&mut self) {
        if let Some(entry) = self.entries.get(self.selected_index) {
            if entry.is_dir {
                self.current_path = entry.path.clone();
                self.entries = FileSystem::list_directory(&self.current_path);
                self.selected_index = 0;
                self.scroll_offset = 0;
            }
        }
    }

    pub fn go_to_parent(&mut self) {
        if let Some(parent) = FileSystem::get_parent_dir(&self.current_path) {
            self.current_path = parent;
            self.entries = FileSystem::list_directory(&self.current_path);
            self.selected_index = 0;
            self.scroll_offset = 0;
        }
    }

    pub fn select_current(&mut self) -> Option<PathBuf> {
        if let Some(entry) = self.entries.get(self.selected_index) {
            if !entry.is_dir {
                self.selected_file = Some(entry.path.clone());
                return self.selected_file.clone();
            }
        }
        None
    }

    pub fn get_selected_file(&self) -> Option<&PathBuf> {
        self.selected_file.as_ref()
    }

    pub fn current_directory(&self) -> &PathBuf {
        &self.current_path
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" File Browser ")
            .borders(Borders::ALL)
            .border_style(Theme::border_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Mostrar ruta actual
        let path_paragraph = Paragraph::new(self.current_path.to_string_lossy().to_string())
            .style(Theme::normal_style());
        frame.render_widget(path_paragraph, inner);

        let list_area = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: inner.height.saturating_sub(2),
        };

        // Derive max_visible from actual area — adapts to any terminal size
        let max_visible = list_area.height.max(1) as usize;
        self.last_max_visible.set(max_visible);

        let visible_entries: Vec<ListItem> = self
            .entries
            .iter()
            .skip(self.scroll_offset)
            .take(max_visible)
            .enumerate()
            .map(|(i, entry)| {
                let actual_index = self.scroll_offset + i;
                let is_selected = actual_index == self.selected_index;

                let icon = if entry.is_dir { "[DIR]" } else { "[FILE]" };
                let content = format!("{} {}", icon, entry.name);

                let style = if is_selected {
                    Theme::selected_style()
                } else if entry.is_dir {
                    Style::default().fg(Theme::accent())
                } else {
                    Theme::normal_style()
                };

                ListItem::new(content).style(style)
            })
            .collect();

        let list = List::new(visible_entries);
        frame.render_widget(list, list_area);

        // Instructions
        let instructions = Paragraph::new(
            "Up/Down: Navigate | Enter: Open/Select | Left: Parent | q: Cancel",
        )
        .style(Theme::warning_style());

        let instr_area = Rect {
            x: inner.x,
            y: inner.y + inner.height - 1,
            width: inner.width,
            height: 1,
        };
        frame.render_widget(instructions, instr_area);
    }
}
