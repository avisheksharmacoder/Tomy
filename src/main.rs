use std::error::Error;
use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

mod app;
mod chat;
mod code;
pub mod completion;
mod editor;
mod explorer;
mod home;
pub mod ruff_service;
mod runner;
mod settings;
mod todo;
pub mod token_counter;

use app::App;

fn main() -> Result<(), Box<dyn Error>> {
    // Check if invoked as secondary terminal runner (e.g. `tomy runner <file_path>`)
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "runner" {
        return runner::run_runner(&args[2]);
    }
    // 1. Setup panic hook so terminal is always cleanly restored if a panic occurs
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        default_panic(panic_info);
    }));

    // 2. Initialize Terminal in Raw Mode, Alternate Screen, and Mouse Capture
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // 3. Application State & Event Loop
    let mut app = App::new();

    while !app.should_quit {
        terminal.draw(|frame| app.render(frame))?;

        // 50ms tick rate for snappy UI feedback
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                _ => {}
            }
        }
    }

    // 4. Restore terminal state before exiting
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
