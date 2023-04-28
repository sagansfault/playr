use std::{
    collections::VecDeque,
    error::Error,
    io::{BufReader, Stdout},
    time::{Duration, Instant},
};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use rand::seq::SliceRandom;
use rodio::Sink;
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, List, ListItem, ListState, Row, Table},
    Frame, Terminal,
};

struct StatefulList<T> {
    state: ListState,
    items: Vec<T>,
}

impl<T> StatefulList<T> {
    fn with_items(items: Vec<T>) -> StatefulList<T> {
        let empty = items.is_empty();
        let mut list = StatefulList {
            state: ListState::default(),
            items,
        };
        list.state.select(if empty { None } else { Some(0) });
        list
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }
}

pub struct App<'a> {
    songs: StatefulList<String>,
    sink: &'a mut Sink,
    queue: VecDeque<String>,
    looping: bool,
    shuffle: bool,
    playing: Option<String>,
}



impl<'a> App<'a> {
    pub fn new(sink: &mut Sink) -> App {
        let mut songs: Vec<String> = vec![];
        if let Ok(paths) = std::fs::read_dir("playrsources") {
            for path in paths {
                if let Some(s) = path
                    .map(|o| o.file_name().into_string().ok())
                    .ok()
                    .flatten()
                {
                    songs.push(s);
                }
            }
        }

        App {
            songs: StatefulList::with_items(songs),
            sink,
            queue: VecDeque::new(),
            looping: false,
            shuffle: false,
            playing: None,
        }
    }

    pub fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        tick_rate: Duration,
    ) -> Result<(), Box<dyn Error>> {
        let mut last_tick = Instant::now();
        let mut rng = rand::thread_rng();
        loop {
            // handle playing the next song and looping
            if self.sink.len() == 0 {
                // playing should still be stored here so can we make sure we dont play the same song twice
                if self.shuffle {
                    let mut previous_or_to_play: String = self.playing.as_ref().map(|v| v.clone()).unwrap_or("".to_string());
                    loop {
                        if self.songs.items.len() <= 1 {
                            break;
                        }
                        if let Some(random_song) = self.songs.items.choose(&mut rng) {
                            if !random_song.eq_ignore_ascii_case(&previous_or_to_play) {
                                previous_or_to_play = random_song.clone();
                                break;
                            }
                        }
                    }
                    self.play(previous_or_to_play);
                } else if self.looping && self.playing.is_some() {
                    let current = self.playing.as_ref().unwrap();
                    self.play(current.clone());
                } else {
                    if let Some(next) = self.queue.pop_front() {
                        self.play(next.clone());
                        self.playing = Some(next);
                    } else {
                        self.playing = None;
                    }
                }
            }

            terminal.draw(|f| ui(f, self))?;

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));
            if crossterm::event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Down => self.songs.next(),
                            KeyCode::Up => self.songs.previous(),
                            KeyCode::Enter => {
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    self.queue_selected();
                                } else {
                                    self.play_selected();
                                }
                                self.sink.play();
                            }
                            KeyCode::Char(' ') => {
                                if self.sink.is_paused() {
                                    self.sink.play();
                                } else {
                                    self.sink.pause();
                                }
                            }
                            KeyCode::Char('=') => {
                                self.looping = !self.looping;
                            }
                            KeyCode::Backspace => {
                                self.sink.stop(); // since we only hold one track in the stream at a time, this works as a skip
                                self.sink.play();
                            },
                            KeyCode::Tab => {
                                self.shuffle = !self.shuffle;
                            },
                            KeyCode::Right => {
                                self.sink.set_volume(
                                    ((self.sink.volume() * 100.0) as usize + 10).min(200) as f32
                                        / 100.0,
                                );
                            }
                            KeyCode::Left => {
                                self.sink.set_volume(
                                    ((self.sink.volume() * 100.0) as usize - 10).max(10) as f32
                                        / 100.0,
                                );
                            }
                            _ => {}
                        }
                    }
                }
            }
            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }
        }
    }

    fn play(&mut self, song: String) {
        let file = std::fs::File::open(format!("playrsources/{}", song)).unwrap();
        self.sink
            .append(rodio::Decoder::new(BufReader::new(file)).unwrap());
        self.playing = Some(song)
    }

    fn play_selected(&mut self) {
        if let Some(selected) = self.get_selected() {
            self.play(selected);
        }
    }

    fn queue_selected(&mut self) {
        if let Some(selected) = self.get_selected() {
            self.queue.push_back(selected);
        }
    }

    fn get_selected(&mut self) -> Option<String> {
        self.songs
            .state
            .selected()
            .map(|ind| self.songs.items.get(ind).map(|v| v.clone()))
            .flatten()
    }
}

fn ui(f: &mut Frame<CrosstermBackend<Stdout>>, app: &mut App) {
    let size = f.size();
    // Main Block
    let block = Block::default().borders(Borders::ALL).title("Playr");
    f.render_widget(block, size);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(f.size());

    // Right 3 chunks
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(40),
                Constraint::Percentage(20),
                Constraint::Percentage(40),
            ]
            .as_ref(),
        )
        .split(chunks[1]);

    let songs: Vec<ListItem> = app
        .songs
        .items
        .iter()
        .map(|e| ListItem::new(e.clone()))
        .collect();
    let songs_block = List::new(songs)
        .block(Block::default().borders(Borders::ALL).title("Songs"))
        .highlight_style(
            Style::default()
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    f.render_stateful_widget(songs_block, chunks[0], &mut app.songs.state);

    let rows: Vec<Row> = vec![
        Row::new(vec![Cell::from("Play"), Cell::from("Enter")]),
        Row::new(vec![Cell::from("Queue"), Cell::from("Shift + Enter")]),
        Row::new(vec![Cell::from("Pause"), Cell::from("Space")]),
        Row::new(vec![Cell::from("Loop"), Cell::from("=")]),
        Row::new(vec![Cell::from("Shuffle"), Cell::from("Tab")]),
        Row::new(vec![Cell::from("Skip"), Cell::from("Backspace")]),
        Row::new(vec![Cell::from("Volume Up"), Cell::from("Right Arrow")]),
        Row::new(vec![Cell::from("Volume Down"), Cell::from("Left Arrow")]),
    ];
    let controls_block = Table::new(rows)
        .block(Block::default().borders(Borders::ALL).title("Controls"))
        .widths(&[Constraint::Percentage(50), Constraint::Percentage(50)]);
    f.render_widget(controls_block, right_chunks[0]);

    let rows: Vec<Row> = vec![
        if app.sink.is_paused() {
            Row::new(vec![Cell::from("Paused"), Cell::from("")])
        } else if app.looping {
            Row::new(vec![
                Cell::from("Looping"),
                Cell::from(format!(
                    "{}",
                    app.playing
                        .as_ref()
                        .map(|p| p.clone())
                        .unwrap_or("None".to_string())
                )),
            ])
        } else if app.shuffle {
            Row::new(vec![Cell::from("Shuffling"), Cell::from("")])
        } else {
            Row::new(vec![
                Cell::from("Playing"),
                Cell::from(format!(
                    "{}",
                    app.playing
                        .as_ref()
                        .map(|p| p.clone())
                        .unwrap_or("None".to_string())
                )),
            ])
        },
        Row::new(vec![
            Cell::from("Volume"),
            Cell::from(format!("{}%", (app.sink.volume() * 100.0) as usize)),
        ]),
    ];
    let status_block = Table::new(rows)
        .block(Block::default().borders(Borders::ALL).title("Status"))
        .widths(&[Constraint::Percentage(50), Constraint::Percentage(50)]);
    f.render_widget(status_block, right_chunks[1]);

    while app.queue.len() > app.sink.len() {
        app.queue.pop_front();
    }
    let queue_list: Vec<ListItem> = app.queue.iter().map(|v| ListItem::new(v.clone())).collect();
    let queue_block =
        List::new(queue_list).block(Block::default().borders(Borders::ALL).title("Queue"));
    f.render_widget(queue_block, right_chunks[2]);
}
