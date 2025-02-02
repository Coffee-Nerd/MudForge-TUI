// main.rs

use std::error::Error;
use std::fs::File;
use std::io;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::Duration;

mod telnet_client;
mod ansi_color;
mod gmcp_store;

use crate::telnet_client::{TelnetClient, TelnetMessage, GroupInfo};
use crate::gmcp_store::GMCPStore;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use log::{debug, error, info, LevelFilter};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::Backend;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use simplelog::{Config, WriteLogger};

/// Holds personal gauge data
#[derive(Clone, Debug)]
pub struct Vitals {
    pub hp: i32,
    pub mana: i32,
    pub movement: i32,
}

#[derive(Clone, Debug)]
pub struct MaxStats {
    pub maxhp: i32,
    pub maxmana: i32,
    pub maxmove: i32,
}

struct AppState {
    mud_output: VecDeque<Vec<Span<'static>>>,
    chat_output: VecDeque<Vec<Span<'static>>>,
    input: String,
    scroll_offset: u16,
    chat_scroll_offset: u16,
    command_history: Vec<String>,
    history_index: Option<usize>,
    common_commands: Vec<String>,

    // Personal GMCP info:
    gmcp_vitals: Option<Vitals>,
    gmcp_maxstats: Option<MaxStats>,
    gmcp_enemy: Option<i32>,           // Enemy gauge from char.status (if needed)
    group_info: Option<GroupInfo>,     // group GMCP info (which includes enemy info)
}

impl AppState {
    fn new() -> Self {
        Self {
            mud_output: VecDeque::new(),
            chat_output: VecDeque::new(),
            input: String::new(),
            scroll_offset: 0,
            chat_scroll_offset: 0,
            command_history: Vec::new(),
            history_index: None,
            common_commands: vec![
                "look".to_string(),
                "inventory".to_string(),
                "say".to_string(),
                "quit".to_string(),
                "help".to_string(),
            ],
            gmcp_vitals: None,
            gmcp_maxstats: None,
            gmcp_enemy: None,
            group_info: None,
        }
    }

    fn add_mud_output(&mut self, line: Vec<Span<'static>>) {
        if self.mud_output.len() > 2000 {
            self.mud_output.pop_front();
        }
        self.mud_output.push_back(line);
    }

    fn add_chat_output(&mut self, line: Vec<Span<'static>>) {
        if self.chat_output.len() > 1000 {
            self.chat_output.pop_front();
        }
        self.chat_output.push_back(line);
    }

    fn scroll_up_main(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }
    fn scroll_down_main(&mut self) {
        if self.scroll_offset < self.mud_output.len() as u16 {
            self.scroll_offset += 1;
        }
    }
    fn scroll_up_chat(&mut self) {
        if self.chat_scroll_offset > 0 {
            self.chat_scroll_offset -= 1;
        }
    }
    fn scroll_down_chat(&mut self) {
        if self.chat_scroll_offset < self.chat_output.len() as u16 {
            self.chat_scroll_offset += 1;
        }
    }

    fn add_to_history(&mut self, cmd: String) {
        if !cmd.trim().is_empty() {
            self.command_history.push(cmd);
        }
        self.history_index = None;
    }

    fn history_up(&mut self) {
        if self.command_history.is_empty() {
            return;
        }
        match self.history_index {
            None => self.history_index = Some(self.command_history.len().saturating_sub(1)),
            Some(0) => {}
            Some(i) => self.history_index = Some(i.saturating_sub(1)),
        }
        if let Some(i) = self.history_index {
            self.input = self.command_history[i].clone();
        }
    }

    fn history_down(&mut self) {
        if self.command_history.is_empty() {
            return;
        }
        match self.history_index {
            None => {}
            Some(i) if i >= self.command_history.len() - 1 => {
                self.history_index = None;
                self.input.clear();
            }
            Some(i) => {
                self.history_index = Some(i + 1);
                if let Some(j) = self.history_index {
                    self.input = self.command_history[j].clone();
                }
            }
        }
    }

    fn autocomplete(&mut self) {
        let prefix = self.input.trim();
        if prefix.is_empty() {
            return;
        }
        let matches: Vec<&String> = self
            .common_commands
            .iter()
            .filter(|cmd| cmd.starts_with(prefix))
            .collect();
        if !matches.is_empty() {
            self.input = matches[0].clone();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Set up logging.
    let file = File::create("mud_tui_debug.log")?;
    WriteLogger::init(LevelFilter::Debug, Config::default(), file)?;
    info!("Starting MUD TUI. Logs in mud_tui_debug.log");

    let (tx, mut rx) = mpsc::channel(100);
    let telnet_client = TelnetClient::new(tx.clone());
    
    // Create the GMCP store.
    let gmcp_store = Arc::new(Mutex::new(GMCPStore::new()));

    // Adjust host and port as needed.
    telnet_client
        .connect("darkwiz.org", "6969", gmcp_store.clone())
        .await
        .map_err(|e| {
            error!("Failed to connect: {}", e);
            e
        })?;

    let app_state = Arc::new(Mutex::new(AppState::new()));
    let ui_state = Arc::clone(&app_state);

    // Spawn a task to handle incoming TelnetMessages and update UI state.
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let mut st = ui_state.lock().await;
            match msg {
                TelnetMessage::MUDOutput(spans) => st.add_mud_output(spans),
                TelnetMessage::ChatMessage(spans) => st.add_chat_output(spans),
                TelnetMessage::Disconnect => {
                    st.add_mud_output(vec![Span::styled(
                        "Disconnected".to_string(),
                        Style::default().fg(Color::Red),
                    )]);
                    break;
                }
                // Update gauge data from Char.Vitals GMCP message.
                TelnetMessage::CharVitals(hp, maxhp, mana, maxmana, moves, maxmoves) => {
                    let line = Span::styled(
                        format!(
                            "GMCP: Char.Vitals => HP {}/{}, Mana {}/{}, Moves {}/{}",
                            hp, maxhp, mana, maxmana, moves, maxmoves
                        ),
                        Style::default().fg(Color::Cyan),
                    );
                    st.add_mud_output(vec![line]);
                    st.gmcp_vitals = Some(Vitals {
                        hp,
                        mana,
                        movement: moves,
                    });
                    st.gmcp_maxstats = Some(MaxStats {
                        maxhp,
                        maxmana,
                        maxmove: maxmoves,
                    });
                }
                TelnetMessage::CharLogin(name) => {
                    let line = Span::styled(
                        format!("GMCP: Char.Login => name={}", name),
                        Style::default().fg(Color::Green),
                    );
                    st.add_mud_output(vec![line]);
                }
                TelnetMessage::RoomInfo(name, zone) => {
                    let line = Span::styled(
                        format!("GMCP: Room.Info => name={}, zone={}", name, zone),
                        Style::default().fg(Color::Magenta),
                    );
                    st.add_mud_output(vec![line]);
                }
            }
        }
    });

    // Set up the TUI.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let (input_tx, mut input_rx) = mpsc::channel(100);
    // Spawn a task for reading keyboard events.
    tokio::spawn(async move {
        loop {
            let ev = tokio::task::spawn_blocking(|| {
                if event::poll(Duration::from_millis(100)).unwrap() {
                    event::read().ok()
                } else {
                    None
                }
            })
            .await
            .unwrap();

            if let Some(e) = ev {
                debug!("Got an event from crossterm: {:?}", e);
                if input_tx.send(e).await.is_err() {
                    break;
                }
            }
        }
    });

    // Main UI loop.
    loop {
        {
            let st = app_state.lock().await;
            terminal.draw(|f| ui_draw(f, &st))?;
        }
        tokio::select! {
            evt = input_rx.recv() => {
                if let Some(e) = evt {
                    let mut st = app_state.lock().await;
                    match e {
                        CEvent::Key(k) => match k.code {
                            KeyCode::Char(c) => { st.input.push(c); }
                            KeyCode::Backspace => { st.input.pop(); }
                            KeyCode::Enter => {
                                let cmd_to_send = st.input.clone();
                                let echo_line = format!("> {}", st.input);
                                st.add_mud_output(vec![Span::styled(echo_line, Style::default().fg(Color::Yellow))]);
                                let input_value = std::mem::take(&mut st.input);
                                st.add_to_history(input_value);
                                st.input.clear();
                                st.history_index = None;
                                drop(st);
                                let telnet_client_clone = telnet_client.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = telnet_client_clone.send_command(&cmd_to_send).await {
                                        error!("Failed to send command: {}", e);
                                    }
                                });
                            }
                            KeyCode::Up => { st.history_up(); }
                            KeyCode::Down => { st.history_down(); }
                            KeyCode::Tab => { st.autocomplete(); }
                            KeyCode::Esc => { info!("ESC pressed, exiting..."); break; }
                            KeyCode::F(1) => { st.scroll_up_chat(); }
                            KeyCode::F(2) => { st.scroll_down_chat(); }
                            KeyCode::PageUp => { st.scroll_up_main(); }
                            KeyCode::PageDown => { st.scroll_down_main(); }
                            _ => {}
                        },
                        CEvent::Mouse(me) => {
                            if let Ok((width, _)) = crossterm::terminal::size() {
                                if me.kind == event::MouseEventKind::ScrollUp {
                                    if me.column < (width * 3) / 4 {
                                        st.scroll_down_main();
                                    } else {
                                        st.scroll_up_chat();
                                    }
                                } else if me.kind == event::MouseEventKind::ScrollDown {
                                    if me.column < (width * 3) / 4 {
                                        st.scroll_up_main();
                                    } else {
                                        st.scroll_down_chat();
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                } else { break; }
            }
            _ = tokio::time::sleep(Duration::from_millis(1)) => {}
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    info!("Application exited gracefully");
    Ok(())
}

/// Renders the UI.
fn ui_draw<B: Backend>(f: &mut ratatui::Frame<B>, st: &AppState) {
    let outer = f.size();
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .constraints([Constraint::Ratio(3, 4), Constraint::Ratio(1, 4)].as_ref())
        .split(outer);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(3), // Gauge area
            Constraint::Length(3), // Input area
        ])
        .split(chunks[0]);
    let main_rect = left_chunks[0];
    let gauge_rect = left_chunks[1];
    let input_rect = left_chunks[2];
    let chat_rect = chunks[1];

    f.render_widget(Clear, main_rect);
    f.render_widget(Clear, gauge_rect);
    f.render_widget(Clear, input_rect);
    f.render_widget(Clear, chat_rect);

    let lines_main: Vec<Line> = st
        .mud_output
        .iter()
        .map(|lv| Line::from(lv.clone()))
        .collect();
    let visible_height_main = main_rect.height.saturating_sub(2);
    let total_main_lines = lines_main.len() as i32;
    let offset = st.scroll_offset as i32;
    let scroll_top_main = if total_main_lines > visible_height_main as i32 {
        (total_main_lines - visible_height_main as i32).saturating_sub(offset)
    } else { 0 }
    .max(0) as u16;
    let mud_par = Paragraph::new(lines_main)
        .block(Block::default().borders(Borders::ALL).title(" MUD Output "))
        .wrap(Wrap { trim: false })
        .scroll((scroll_top_main, 0));
    f.render_widget(mud_par, main_rect);

    let lines_chat: Vec<Line> = st
        .chat_output
        .iter()
        .map(|lv| Line::from(lv.clone()))
        .collect();
    let visible_height_chat = chat_rect.height.saturating_sub(2);
    let total_chat_lines = lines_chat.len() as i32;
    let offset_chat = st.chat_scroll_offset as i32;
    let scroll_top_chat = if total_chat_lines > visible_height_chat as i32 {
        (total_chat_lines - visible_height_chat as i32).saturating_sub(offset_chat)
    } else { 0 }
    .max(0) as u16;
    let chat_par = Paragraph::new(lines_chat)
        .block(Block::default().borders(Borders::ALL).title(" Chat "))
        .wrap(Wrap { trim: false })
        .scroll((scroll_top_chat, 0));
    f.render_widget(chat_par, chat_rect);

    let mut gauge_lines = Vec::new();
    if let (Some(vitals), Some(maxstats)) = (&st.gmcp_vitals, &st.gmcp_maxstats) {
        gauge_lines.push(Line::from(render_gauge("HP", vitals.hp, maxstats.maxhp)));
        gauge_lines.push(Line::from(render_gauge("MN", vitals.mana, maxstats.maxmana)));
        gauge_lines.push(Line::from(render_gauge("MV", vitals.movement, maxstats.maxmove)));
    } else {
        gauge_lines.push(Line::from(Span::raw("No GMCP gauge data")));
    }
    let gauge_par = Paragraph::new(gauge_lines)
        .block(Block::default().borders(Borders::ALL).title(" Gauges "));
    f.render_widget(gauge_par, gauge_rect);

    let inp_par = Paragraph::new(st.input.as_str())
        .block(Block::default().borders(Borders::ALL).title(" Input "))
        .style(Style::default().fg(Color::Yellow))
        .wrap(Wrap { trim: false });
    f.render_widget(inp_par, input_rect);

    let cursor_x =
        input_rect.x + (st.input.len() as u16).min(input_rect.width.saturating_sub(2)) + 1;
    let cursor_y = input_rect.y + 1;
    if cursor_x < f.size().width && cursor_y < f.size().height {
        f.set_cursor(cursor_x, cursor_y);
    }
}

/// Renders a gauge bar as a string.
fn render_gauge(label: &str, current: i32, max: i32) -> String {
    let bar_length = 20;
    let percentage = if max > 0 { current as f64 / max as f64 } else { 0.0 };
    let filled_length = (percentage * bar_length as f64).round() as usize;
    format!(
        "{}: [{}{}] {}/{}",
        label,
        "=".repeat(filled_length),
        " ".repeat(bar_length - filled_length),
        current,
        max
    )
}
