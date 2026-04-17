use crate::ui::theme::Theme;
use crate::utils::logging::{LogEntry, LogLevel};
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::collections::VecDeque;

pub struct StatusPanel {
    logs: VecDeque<LogEntry>,
    scroll_offset: usize,
}

impl StatusPanel {
    pub fn new() -> Self {
        Self {
            logs: VecDeque::new(),
            scroll_offset: 0,
        }
    }

    pub fn add_log(&mut self, entry: LogEntry) {
        self.logs.push_back(entry);
        // Auto-scroll: maintain maximum 100 entries using O(1) pop_front
        if self.logs.len() > 100 {
            self.logs.pop_front();
        }
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    pub fn scroll_down(&mut self) {
        let max_scroll = self.logs.len().saturating_sub(10);
        if self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.logs.len()
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Status / Logs ")
            .borders(Borders::ALL)
            .border_style(Theme::border_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let visible_logs: Vec<Line> = self
            .logs
            .iter()
            .skip(self.scroll_offset)
            .take(inner.height as usize)
            .map(|entry| {
                // Dynamic color mapping by log level — not hardcoded to success()
                let level_color = match entry.level {
                    LogLevel::Info  => Theme::success(),
                    LogLevel::Warn  => Theme::warning(),
                    LogLevel::Error => Theme::error(),
                };

                Line::from(vec![
                    Span::styled(
                        format!("[{}] ", entry.timestamp),
                        Style::default().fg(Theme::foreground()),
                    ),
                    Span::styled(
                        format!("{:?}: ", entry.level),
                        Style::default().fg(level_color),
                    ),
                    Span::styled(&entry.message, Style::default().fg(Theme::foreground())),
                ])
            })
            .collect();

        let paragraph = Paragraph::new(visible_logs);
        frame.render_widget(paragraph, inner);
    }
}
