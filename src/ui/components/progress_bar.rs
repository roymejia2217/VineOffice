use crate::ui::theme::Theme;
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

#[derive(Clone)]
pub struct SubProgress {
    pub current: usize,
    pub total: usize,
    pub detail: String,
}

pub struct StepsProgress {
    completed_steps: usize,
    total_steps: usize,
    step_name: String,
    sub_progress: Option<SubProgress>,
}

impl StepsProgress {
    pub fn new(
        completed_steps: usize,
        total_steps: usize,
        step_name: impl Into<String>,
        sub_progress: Option<SubProgress>,
    ) -> Self {
        Self {
            completed_steps,
            total_steps,
            step_name: step_name.into(),
            sub_progress,
        }
    }
}

impl Widget for StepsProgress {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 3 {
            return;
        }

        let sub_fraction = match &self.sub_progress {
            Some(sub) => sub.current as f32 / sub.total.max(1) as f32,
            None => 0.0,
        };
        let progress = (self.completed_steps as f32 + sub_fraction) / self.total_steps as f32;

        let total_width = (area.width - 2) as usize;
        let filled_width = (total_width as f32 * progress) as usize;
        let empty_width = total_width.saturating_sub(filled_width);

        let filled = "█".repeat(filled_width);
        let empty = "░".repeat(empty_width);
        let percentage = (progress * 100.0) as u8;

        let current_step_display = (self.completed_steps + 1).min(self.total_steps);

        let label = match &self.sub_progress {
            Some(sub) => format!(
                "[{}{}] {}% - Step {}/{}: {} ({}/{}: {})",
                filled,
                empty,
                percentage,
                current_step_display,
                self.total_steps,
                self.step_name,
                sub.current,
                sub.total,
                sub.detail
            ),
            None => format!(
                "[{}{}] {}% - Step {}/{}: {}",
                filled, empty, percentage, current_step_display, self.total_steps, self.step_name
            ),
        };

        let display = if label.len() > area.width as usize {
            label.chars().take(area.width as usize).collect::<String>()
        } else {
            label
        };

        buf.set_string(
            area.x,
            area.y,
            display,
            ratatui::style::Style::default().fg(Theme::accent()),
        );
    }
}
