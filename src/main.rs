mod app;
mod ui;
mod filetree;
mod search;

use anyhow::Result;
use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{env, io, time::Duration};

fn main() -> Result<()> {
    let vault_path = env::args().nth(1).unwrap_or_else(|| ".".to_string());

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(vault_path)?;
    let result = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {e}");
    }
    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    // Initial draw
    terminal.draw(|f| ui::draw(f, app))?;

    loop {
        // Tick debouncer for global search
        let vault = app.vault_path.clone();
        let search_fired = app.global_search.tick_debounce(&vault);

        let timeout = if app.global_search.dirty {
            Duration::from_millis(16)
        } else {
            Duration::from_secs(3600)
        };

        if event::poll(timeout)? {
            let mut should_quit = false;
            // Drain ALL queued events before redrawing — eliminates input backlog lag
            loop {
                match event::read()? {
                    Event::Key(key) => {
                        if app.handle_key(key)? {
                            should_quit = true;
                        }
                    }
                    Event::Mouse(mouse) => {
                        app.handle_mouse(mouse);
                    }
                    Event::Resize(_, _) => {}
                    _ => {}
                }
                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
            if should_quit {
                return Ok(());
            }
        } else if !search_fired {
            continue;
        }

        terminal.draw(|f| ui::draw(f, app))?;
    }
}
