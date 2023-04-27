use std::{error::Error, time::Duration};

use app::App;
use crossterm::{
    event::EnableMouseCapture,
    execute,
    terminal::{enable_raw_mode, EnterAlternateScreen},
};
use tui::{backend::CrosstermBackend, Terminal};

mod app;

fn main() -> Result<(), Box<dyn Error>> {
    let (_stream, handle) = rodio::OutputStream::try_default().unwrap();
    let mut sink = rodio::Sink::try_new(&handle).unwrap();

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tick_rate = Duration::from_millis(250);
    let mut app = App::new(&mut sink);
    let _ = app.run(&mut terminal, tick_rate);

    Ok(())
}
