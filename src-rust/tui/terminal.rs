use std::io::{self, Stdout};
use std::sync::Arc;

use crossterm::{
    cursor,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

type PanicHook = Arc<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync + 'static>;

pub struct PanicRestoreHookGuard {
    previous: Option<PanicHook>,
}

impl PanicRestoreHookGuard {
    pub fn install() -> Self {
        let previous: PanicHook = std::panic::take_hook().into();
        let previous_for_hook = previous.clone();

        std::panic::set_hook(Box::new(move |info| {
            let mut stdout = io::stdout();
            let _ = restore_stdout_best_effort(&mut stdout);
            previous_for_hook(info);
        }));

        Self {
            previous: Some(previous),
        }
    }
}

impl Drop for PanicRestoreHookGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            std::panic::set_hook(Box::new(move |info| previous(info)));
        }
    }
}

fn restore_stdout_best_effort(stdout: &mut Stdout) -> Result<(), String> {
    let mut first_err: Option<String> = None;

    if let Err(e) = disable_raw_mode() {
        first_err.get_or_insert(e.to_string());
    }
    if let Err(e) = execute!(
        stdout,
        cursor::Show,
        LeaveAlternateScreen,
        DisableMouseCapture
    ) {
        first_err.get_or_insert(e.to_string());
    }

    match first_err {
        Some(err) => Err(format!("terminal error: {err}")),
        None => Ok(()),
    }
}

pub struct TuiTerminal {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    active: bool,
}

impl TuiTerminal {
    pub fn new() -> Result<Self, String> {
        let mut stdout = io::stdout();
        enable_raw_mode().map_err(|e| format!("terminal error: {e}"))?;
        if let Err(e) = execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            cursor::Hide
        ) {
            let _ = restore_stdout_best_effort(&mut stdout);
            return Err(format!("terminal error: {e}"));
        }

        let terminal = match Terminal::new(CrosstermBackend::new(stdout)) {
            Ok(terminal) => terminal,
            Err(e) => {
                let mut stdout = io::stdout();
                let _ = restore_stdout_best_effort(&mut stdout);
                return Err(format!("terminal error: {e}"));
            }
        };

        Ok(Self {
            terminal,
            active: true,
        })
    }

    pub fn draw<F>(&mut self, f: F) -> Result<(), String>
    where
        F: FnOnce(&mut ratatui::Frame<'_>),
    {
        self.terminal
            .draw(f)
            .map(|_| ())
            .map_err(|e| format!("terminal error: {e}"))
    }

    pub fn restore_best_effort(&mut self) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }

        let mut first_err: Option<String> = None;

        if let Err(e) = disable_raw_mode() {
            first_err.get_or_insert(e.to_string());
        }
        if let Err(e) = execute!(
            self.terminal.backend_mut(),
            cursor::Show,
            LeaveAlternateScreen,
            DisableMouseCapture
        ) {
            first_err.get_or_insert(e.to_string());
        }
        let _ = self.terminal.show_cursor();

        match first_err {
            Some(err) => Err(format!("terminal error: {err}")),
            None => {
                self.active = false;
                Ok(())
            }
        }
    }
}

impl Drop for TuiTerminal {
    fn drop(&mut self) {
        let _ = self.restore_best_effort();
    }
}
