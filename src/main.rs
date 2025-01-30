// src/main.rs

mod ansi_color;
mod telnet_client;
use std::collections::VecDeque;
use crate::telnet_client::{TelnetClient, TelnetMessage};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use std::error::Error;
use std::io;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::time::{self, Duration};
use log::{info, error};

struct AppState {
    mud_output: VecDeque<Vec<(String, Color)>>,
    chat_output: VecDeque<String>,
    input: String,
}

impl AppState {
    const MAX_MUD_OUTPUT: usize = 1000;
    const MAX_CHAT_OUTPUT: usize = 100;

    fn new() -> Self {
        Self {
            mud_output: VecDeque::with_capacity(Self::MAX_MUD_OUTPUT),
            chat_output: VecDeque::with_capacity(Self::MAX_CHAT_OUTPUT),
            input: String::new(),
        }
    }

    fn add_mud_output(&mut self, text: Vec<(String, Color)>) {
        if self.mud_output.len() == Self::MAX_MUD_OUTPUT {
            self.mud_output.pop_front();
        }
        self.mud_output.push_back(text);
    }

    fn add_chat_output(&mut self, text: String) {
        if self.chat_output.len() == Self::MAX_CHAT_OUTPUT {
            self.chat_output.pop_front();
        }
        self.chat_output.push_back(text);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting MUD TUI Application");

    let (tx, mut rx) = mpsc::channel(100);
    let telnet_client = Arc::new(Mutex::new(TelnetClient::new(tx.clone())));
    
    {
        let mut client = telnet_client.lock().await;
        client.connect("darkwiz.org", "6969").await.map_err(|e| {
            error!("Failed to connect: {}", e);
            e
        })?;
    }

    let telnet_client_clone = Arc::clone(&telnet_client);
    let _telnet_client_task = tokio::spawn(async move {
        let mut client = telnet_client_clone.lock().await;
        client.run().await;
    });

    let app_state = Arc::new(Mutex::new(AppState::new()));
    let ui_state = Arc::clone(&app_state);

    let _ui_handle = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let mut state = ui_state.lock().await;
            match message {
                TelnetMessage::MUDOutput(spans) => {
                    state.add_mud_output(spans);
                }
                TelnetMessage::ChatMessage(text) => {
                    state.add_chat_output(text);
                }
                TelnetMessage::Disconnect => {
                    state.add_mud_output(vec![("Disconnected from MUD.".to_string(), Color::Red)]);
                    break;
                }
            }
        }
    });

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

// Create a channel for input events
let (input_tx, mut input_rx) = mpsc::channel(100);

// Spawn a thread to capture input events using compatible crossterm API
tokio::spawn(async move {
    loop {
        // Use spawn_blocking for event polling
        let event = tokio::task::spawn_blocking(|| {
            if event::poll(Duration::from_millis(100)).unwrap() {
                event::read().ok()
            } else {
                None
            }
        }).await.unwrap();

        if let Some(evt) = event {
            if input_tx.send(evt).await.is_err() {
                break;
            }
        }
    }
});

    let telnet_client_clone_for_send = Arc::clone(&telnet_client);

    loop {
        let (mud_output, chat_output, input) = {
            let state = app_state.lock().await;
            (
                state.mud_output.clone(),
                state.chat_output.clone(),
                state.input.clone(),
            )
        };

        terminal.draw(|f| ui_draw(f, &mud_output, &chat_output, &input))?;

        tokio::select! {
            maybe_event = input_rx.recv() => {
                if let Some(event) = maybe_event {
                    let mut state = app_state.lock().await;
                    match event {
                        CEvent::Key(key) => match key.code {
                            KeyCode::Char(c) => {
                                state.input.push(c);
                            }
                            KeyCode::Backspace => {
                                state.input.pop();
                            }
                            KeyCode::Enter => {
                                let command = state.input.clone() + "\r\n";
                                let input_clone = state.input.clone();
                                state.mud_output.push_back(vec![(format!("> {}", input_clone), Color::Yellow)]);
                                state.input.clear();

                                let telnet_client_clone = Arc::clone(&telnet_client_clone_for_send);
                                tokio::spawn(async move {
                                    let mut client = telnet_client_clone.lock().await;
                                    if let Err(e) = client.send_command(&command).await {
                                        error!("Failed to send command: {}", e);
                                    }
                                });
                            }
                            KeyCode::Esc => {
                                info!("Exit key pressed");
                                break;
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                } else {
                    break;
                }
            }
            _ = time::sleep(Duration::from_millis(100)) => {}
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    info!("Application exited gracefully");
    Ok(())
}

fn ui_draw<B: Backend>(
    f: &mut ratatui::Frame<B>,
    mud_output: &VecDeque<Vec<(String, Color)>>,
    chat_output: &VecDeque<String>,
    input: &str,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref())
        .split(f.size());

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .split(chunks[0]);

    let mud_lines: Vec<Line> = mud_output.iter().map(|line| {
        Line::from(
            line.iter()
                .map(|(text, color)| Span::styled(text, Style::default().fg(*color)))
                .collect::<Vec<_>>()
        )
    }).collect();

    let mud_paragraph = Paragraph::new(mud_lines)
        .block(Block::default().borders(Borders::ALL).title("MUD Output"))
        .wrap(Wrap { trim: false });
    f.render_widget(mud_paragraph, main_chunks[0]);

    let chat_text = chat_output.iter().cloned().collect::<Vec<String>>().join("\n");
    let chat_paragraph = Paragraph::new(chat_text)
        .block(Block::default().borders(Borders::ALL).title("Chat"))
        .wrap(Wrap { trim: false });
    f.render_widget(chat_paragraph, main_chunks[1]);

    let input_paragraph = Paragraph::new(input)
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title("Input"))
        .wrap(Wrap { trim: false });
    f.render_widget(input_paragraph, chunks[1]);

    f.set_cursor(chunks[1].x + input.len() as u16 + 1, chunks[1].y + 1);
}