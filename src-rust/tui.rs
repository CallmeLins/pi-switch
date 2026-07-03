mod app;
mod data;
mod form;
mod i18n;
mod route;
mod terminal;
mod text_edit;
mod theme;
mod ui;

use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyEventKind};

use app::App;
use data::UiData;
use terminal::{PanicRestoreHookGuard, TuiTerminal};

const TUI_TICK_RATE: Duration = Duration::from_millis(200);

pub fn run_tui() -> Result<(), String> {
    let _panic_guard = PanicRestoreHookGuard::install();
    let mut terminal = TuiTerminal::new()?;
    let mut app = App::new(UiData::load());

    let mut last_tick = Instant::now();
    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        let timeout = TUI_TICK_RATE.saturating_sub(last_tick.elapsed());
        if event::poll(timeout).map_err(|e| format!("terminal error: {e}"))? {
            match event::read().map_err(|e| format!("terminal error: {e}"))? {
                Event::Key(key) if key.kind != KeyEventKind::Release => app.on_key(key),
                Event::Mouse(mouse) => app.on_mouse(mouse),
                _ => {}
            }
        }
        if last_tick.elapsed() >= TUI_TICK_RATE {
            app.on_tick();
            last_tick = Instant::now();
        }
    }

    let restore_result = terminal.restore_best_effort();

    // Auto-stop the proxy daemon on exit only if this TUI session started it. A daemon
    // started independently (e.g. `pi-switch proxy start --daemon` from the command line)
    // is left running.
    if app.proxy_started_by_tui {
        let _ = crate::daemon::daemon_stop(&crate::daemon::PROXY);
    }

    restore_result?;
    Ok(())
}
