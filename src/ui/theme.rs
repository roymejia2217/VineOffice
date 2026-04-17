use ratatui::style::{Color, Modifier, Style};

pub struct Theme;

impl Theme {
    pub fn foreground() -> Color {
        Color::White
    }

    pub fn accent() -> Color {
        Color::Cyan
    }

    pub fn success() -> Color {
        Color::Green
    }

    pub fn warning() -> Color {
        Color::Yellow
    }

    pub fn error() -> Color {
        Color::Red
    }

    pub fn title_style() -> Style {
        Style::default()
            .fg(Self::accent())
            .add_modifier(Modifier::BOLD)
    }

    pub fn selected_style() -> Style {
        Style::default()
            .bg(Self::accent())
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    }

    pub fn normal_style() -> Style {
        Style::default().fg(Self::foreground())
    }

    pub fn success_style() -> Style {
        Style::default()
            .fg(Self::success())
            .add_modifier(Modifier::BOLD)
    }

    pub fn error_style() -> Style {
        Style::default()
            .fg(Self::error())
            .add_modifier(Modifier::BOLD)
    }

    pub fn warning_style() -> Style {
        Style::default()
            .fg(Self::warning())
            .add_modifier(Modifier::BOLD)
    }

    pub fn border_style() -> Style {
        Style::default().fg(Self::accent())
    }

    pub fn accent_style() -> Style {
        Style::default().fg(Self::accent())
    }
}
