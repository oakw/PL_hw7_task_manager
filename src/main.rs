use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use rusqlite::{Connection, Result};
use std::{error::Error, io, time::Duration};

mod app;

// Start the app.
// This and the rest of code heavily based on:
// https://github.com/ratatui-org/ratatui/blob/main/examples/list.rs
pub fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initialize connection to the database
    let storage = app::storage::Storage {
        db_con: Connection::open("database.db").expect("Failed to open the DB file"),
    };
    storage.create_table_if_not_exists();

    // Create an app with 250 ms tick
    let tick_rate = Duration::from_millis(250);
    let app = app::ui::App::new(&storage);
    let res = app::ui::run_app(&mut terminal, app, tick_rate);

    // Restore previous terminal state after exit
    // Copied from example
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}
