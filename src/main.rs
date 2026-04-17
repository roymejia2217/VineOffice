mod app;
mod core;
mod ui;
mod utils;

use app::App;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;
use tracing::{error, info};

/// TerminalGuard ensures terminal state is restored even on panic
struct TerminalGuard {
    terminal: Option<Terminal<CrosstermBackend<io::Stdout>>>,
    log_file: std::path::PathBuf,
    _log_guard: tracing_appender::non_blocking::WorkerGuard,
}

impl TerminalGuard {
    fn new() -> anyhow::Result<Self> {
        let (log_file, log_guard) = utils::logging::LogConfig::init()?;
        info!("VineOffice - Log initialized at: {:?}", log_file);

        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal: Some(terminal),
            log_file,
            _log_guard: log_guard,
        })
    }

    fn cleanup(&mut self) -> anyhow::Result<()> {
        disable_raw_mode()?;
        if let Some(term) = self.terminal.take() {
            drop(term);
            execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
            execute!(io::stdout(), crossterm::cursor::Show)?;
        }
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.terminal.is_some() {
            let _ = self.cleanup();
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut guard = TerminalGuard::new()?;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let terminal = guard.terminal.as_mut().unwrap();
        let mut app = App::new();
        app.run(terminal)
    }));

    guard.cleanup()?;

    match result {
        Ok(Ok(_)) => {
            info!("VineOffice - Application exited successfully");
            println!("Installation finished. Check logs at: {:?}", guard.log_file);
            Ok(())
        }
        Ok(Err(e)) => {
            error!("Application error: {}", e);
            eprintln!("Error: {}", e);
            Err(e)
        }
        Err(panic_info) => {
            let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic occurred".to_string()
            };
            error!("Panic: {}", panic_msg);
            eprintln!("Fatal error: {}", panic_msg);
            eprintln!("Terminal state has been restored.");
            Err(anyhow::anyhow!("Fatal panic: {}", panic_msg))
        }
    }
}
