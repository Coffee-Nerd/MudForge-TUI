// telnet_client.rs

use crate::ansi_color::{parse_ansi_codes, COLOR_MAP, strip_mxp_tags};
use crate::gmcp_store::GMCPStore;
use log::{debug, error, info};
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{mpsc::Sender, Mutex};
use tokio::time::{timeout, Duration};

use libmudtelnet::events::{TelnetEvents, TelnetSubnegotiation};
use libmudtelnet::Parser;

////////////////////////////////////////////////////////////////////////////////////////////////////
// Telnet negotiation constants
////////////////////////////////////////////////////////////////////////////////////////////////////
const IAC: u8 = 255;
const WILL: u8 = 251;
const SB: u8 = 250;
const SE: u8 = 240;
const TELOPT_GMCP: u8 = 201;

////////////////////////////////////////////////////////////////////////////////////////////////////
// GMCP data structures for known packages.
////////////////////////////////////////////////////////////////////////////////////////////////////
#[derive(Debug, Deserialize)]
pub struct CharLogin {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct CharVitals {
    pub hp: i32,
    pub maxhp: i32,
    pub mana: i32,
    pub maxmana: i32,
    pub moves: i32,
    pub maxmoves: i32,
}

#[derive(Debug, Deserialize)]
pub struct RoomInfo {
    pub num: i32,
    pub name: String,
    pub zone: String,
}

#[derive(Debug, Deserialize)]
pub struct CommChannel {
    pub chan: String,
    pub msg: String,
    pub player: String,
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// TelnetMessage: extended to handle GMCP messages.
////////////////////////////////////////////////////////////////////////////////////////////////////
#[derive(Debug, Clone)]
pub enum TelnetMessage {
    MUDOutput(Vec<Span<'static>>),
    ChatMessage(Vec<Span<'static>>),
    Disconnect,
    CharLogin(String),                   // (name)
    CharVitals(i32, i32, i32, i32, i32, i32), // (hp, maxhp, mana, maxmana, moves, maxmoves)
    RoomInfo(String, String),            // (room name, zone)
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// TelnetClient struct and implementation.
////////////////////////////////////////////////////////////////////////////////////////////////////
#[derive(Clone)]
pub struct TelnetClient {
    parser: Arc<Mutex<Parser>>,
    write_half: Arc<Mutex<Option<OwnedWriteHalf>>>,
    sender: Sender<TelnetMessage>,
}

impl TelnetClient {
    pub fn new(sender: Sender<TelnetMessage>) -> Self {
        Self {
            parser: Arc::new(Mutex::new(Parser::new())),
            write_half: Arc::new(Mutex::new(None)),
            sender,
        }
    }

    /// Connect to the server and start the read loop.
    /// The gmcp_store is passed in so that incoming GMCP data can be saved.
    pub async fn connect(&self, host: &str, port: &str, gmcp_store: Arc<Mutex<GMCPStore>>) -> Result<(), String> {
        let addr_str = format!("{}:{}", host, port);
        let stream = TcpStream::connect(&addr_str)
            .await
            .map_err(|e| format!("Connection failed: {}", e))?;
        info!("Connected to {}", addr_str);

        let (read_half, write_half) = stream.into_split();
        {
            let mut w = self.write_half.lock().await;
            *w = Some(write_half);
        }
        // Send GMCP negotiation (IAC WILL TELOPT_GMCP)
        self.enable_gmcp().await?;

        // Send additional GMCP requests to prompt data from the server.
        self.fetch_all().await?;

        let parser_clone = Arc::clone(&self.parser);
        let tx_clone = self.sender.clone();
        let write_half_clone = Arc::clone(&self.write_half);
        let gmcp_store_clone = gmcp_store.clone();

        tokio::spawn(async move {
            run_read_loop(read_half, parser_clone, write_half_clone, tx_clone, gmcp_store_clone).await;
        });

        Ok(())
    }

    /// Sends IAC WILL TELOPT_GMCP to enable GMCP.
    pub async fn enable_gmcp(&self) -> Result<(), String> {
        let gmcp_enable = [IAC, WILL, TELOPT_GMCP];
        let mut w = self.write_half.lock().await;
        if let Some(ref mut write_half) = *w {
            write_half.write_all(&gmcp_enable).await.map_err(|e| format!("Failed to enable GMCP: {}", e))?;
            debug!("Sent GMCP negotiation: IAC WILL TELOPT_GMCP");
            Ok(())
        } else {
            Err("No write half available".to_string())
        }
    }

    /// Sends a GMCP subnegotiation packet.
    pub async fn send_gmcp_subneg(&self, msg: &str) -> Result<(), String> {
        let mut packet = vec![IAC, SB, TELOPT_GMCP];
        packet.extend_from_slice(msg.as_bytes());
        packet.extend_from_slice(&[IAC, SE]);

        let mut w = self.write_half.lock().await;
        if let Some(ref mut write_half) = *w {
            write_half.write_all(&packet).await.map_err(|e| e.to_string())?;
            debug!("Sent GMCP subnegotiation: {}", msg);
            Ok(())
        } else {
            Err("No write half available".into())
        }
    }

    /// Sends several GMCP commands to fetch server data.
    pub async fn fetch_all(&self) -> Result<(), String> {
        self.send_gmcp_subneg("config compact").await?;
        self.send_gmcp_subneg("config prompt").await?;
        self.send_gmcp_subneg("config xterm yes").await?;
        self.send_gmcp_subneg("request char").await?;
        self.send_gmcp_subneg("request room").await?;
        self.send_gmcp_subneg("request area").await?;
        self.send_gmcp_subneg("request quest").await?;
        self.send_gmcp_subneg("request group").await?;
        Ok(())
    }

    /// Sends a normal text command (e.g. user input) to the server.
    pub async fn send_command(&self, cmd: &str) -> Result<(), String> {
        let cmd = format!("{}\r\n", cmd.trim());
        debug!("send_command(): sending {:?}", cmd.escape_default());
        let mut w = self.write_half.lock().await;
        let some_wh = match w.as_mut() {
            Some(wh) => wh,
            None => {
                error!("send_command(): Not connected (no write half)");
                return Err("Not connected".to_string());
            }
        };
        let result = timeout(Duration::from_secs(5), some_wh.write_all(cmd.as_bytes())).await;
        match result {
            Ok(Ok(())) => {
                debug!("send_command(): success writing {} bytes", cmd.len());
                Ok(())
            }
            Ok(Err(e)) => {
                error!("Write error: {}", e);
                Err(e.to_string())
            }
            Err(_) => {
                error!("Timed out writing command to server");
                Err("Write timed out".to_string())
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Read loop and GMCP handling.
////////////////////////////////////////////////////////////////////////////////////////////////////
async fn run_read_loop(
    mut r: OwnedReadHalf,
    parser_arc: Arc<Mutex<Parser>>,
    write_half_arc: Arc<Mutex<Option<OwnedWriteHalf>>>,
    tx: Sender<TelnetMessage>,
    gmcp_store: Arc<Mutex<GMCPStore>>,
) {
    let mut buf = [0u8; 8192];
    loop {
        match r.read(&mut buf).await {
            Ok(0) => {
                debug!("Server closed connection");
                let _ = tx.send(TelnetMessage::Disconnect).await;
                break;
            }
            Ok(n) => {
                debug!("Read {} bytes from server", n);
                let raw_bytes = buf[..n].to_vec();
                debug!("Raw bytes: {:?}", raw_bytes);

                // First, try to get events from the parser.
                let mut events = {
                    let mut p = parser_arc.lock().await;
                    p.receive(&raw_bytes)
                };
                debug!("Parsed events from parser: {:?}", events);

                // Fallback: if no events were returned, manually extract GMCP subnegotiations.
                let fallback_events = extract_gmcp_subnegotiations(&raw_bytes);
                if !fallback_events.is_empty() {
                    debug!("Fallback extracted {} GMCP subnegotiation event(s)", fallback_events.len());
                    events.extend(fallback_events);
                }

                // Process all events.
                for ev in events {
                    handle_event(ev, &tx, &write_half_arc, gmcp_store.clone()).await;
                }
            }
            Err(e) => {
                error!("Telnet read error: {}", e);
                let _ = tx.send(TelnetMessage::Disconnect).await;
                break;
            }
        }
    }
}

/// Manually scan raw bytes for GMCP subnegotiation sequences.
/// Returns a vector of TelnetEvents::Subnegotiation events.
fn extract_gmcp_subnegotiations(raw: &[u8]) -> Vec<TelnetEvents> {
    let mut events = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == IAC {
            if i + 1 < raw.len() && raw[i + 1] == SB {
                if i + 2 < raw.len() && raw[i + 2] == TELOPT_GMCP {
                    // Found start of GMCP subnegotiation.
                    let start = i + 3;
                    let mut end = start;
                    // Look for the terminating IAC SE sequence.
                    while end + 1 < raw.len() {
                        if raw[end] == IAC && raw[end + 1] == SE {
                            break;
                        }
                        end += 1;
                    }
                    if end + 1 < raw.len() {
                        let buffer = raw[start..end].to_vec();
                        debug!("Manually extracted GMCP subnegotiation buffer: {:?}", buffer);
                        // Create a fake TelnetSubnegotiation event.
                        events.push(TelnetEvents::Subnegotiation(TelnetSubnegotiation {
                            option: TELOPT_GMCP,
                            buffer: buffer.into(),
                        }));
                        i = end + 2;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    events
}

/// Parses a GMCP message into a package (dot‑separated string) and a JSON value.
fn parse_gmcp(data: &str) -> Option<(String, Value)> {
    let trimmed = data.trim();
    if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
        if let Value::Object(map) = &val {
            if map.len() == 1 {
                let (package, value) = map.iter().next().unwrap();
                return Some((package.clone(), value.clone()));
            }
        }
    }
    // Fallback: split on the first whitespace.
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    if let Some(package) = parts.next() {
        if let Some(json_part) = parts.next() {
            if let Ok(value) = serde_json::from_str::<Value>(json_part.trim()) {
                return Some((package.to_string(), value));
            }
        }
    }
    None
}

/// Tries to parse known GMCP modules and return a corresponding TelnetMessage.
fn parse_known_gmcp_modules(gmcp_str: &str) -> Option<TelnetMessage> {
    if let Some((package, value)) = parse_gmcp(gmcp_str) {
        match package.as_str() {
            "char.login" => {
                if let Ok(obj) = serde_json::from_value::<CharLogin>(value) {
                    return Some(TelnetMessage::CharLogin(obj.name));
                }
            }
            "char.vitals" => {
                if let Ok(obj) = serde_json::from_value::<CharVitals>(value) {
                    return Some(TelnetMessage::CharVitals(
                        obj.hp,
                        obj.maxhp,
                        obj.mana,
                        obj.maxmana,
                        obj.moves,
                        obj.maxmoves,
                    ));
                }
            }
            "room.info" => {
                if let Ok(obj) = serde_json::from_value::<RoomInfo>(value) {
                    return Some(TelnetMessage::RoomInfo(obj.name, obj.zone));
                }
            }
            "comm.channel" => {
                if let Ok(cc) = serde_json::from_value::<CommChannel>(value) {
                    let parsed_msg = parse_gmcp_message(&cc.msg);
                  //  let mut chat_spans = vec![Span::styled(
                  //      format!("[{}] {}: ", cc.chan, cc.player),
                  //      Style::default().fg(Color::Green),
                  //  )];
                  //  chat_spans.extend(parsed_msg);
                    return Some(TelnetMessage::ChatMessage(parsed_msg));
                }
            }
            _ => {}
        }
    }
    None
}

/// Parses inline color markers (like "$R" for red) inside a GMCP message.
pub fn parse_gmcp_message(msg: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current_text = String::new();
    let mut current_color = Color::White;
    let mut chars = msg.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '$' {
            if let Some(&next_ch) = chars.peek() {
                let new_color = match next_ch {
                    'G' => Some(Color::Rgb(0, 255, 0)),
                    'M' => Some(Color::Rgb(255, 0, 255)),
                    'R' => Some(Color::Rgb(255, 0, 0)),
                    'Y' => Some(Color::Rgb(255, 255, 0)),
                    'B' => Some(Color::Rgb(0, 0, 255)),
                    'C' => Some(Color::Rgb(0, 255, 255)),
                    'w' | 'W' => Some(Color::Rgb(255, 255, 255)),
                    _ => None,
                };
                if let Some(col) = new_color {
                    if !current_text.is_empty() {
                        spans.push(Span::styled(current_text.clone(), Style::default().fg(current_color)));
                        current_text.clear();
                    }
                    chars.next(); // Consume the color code.
                    current_color = col;
                    continue;
                } else {
                    current_text.push(ch);
                }
            } else {
                current_text.push(ch);
            }
        } else {
            current_text.push(ch);
        }
    }
    if !current_text.is_empty() {
        spans.push(Span::styled(current_text, Style::default().fg(current_color)));
    }
    spans
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Handle events received from the Telnet parser (or from our manual extraction).
////////////////////////////////////////////////////////////////////////////////////////////////////
async fn handle_event(
    event: TelnetEvents,
    tx: &Sender<TelnetMessage>,
    write_half_arc: &Arc<Mutex<Option<OwnedWriteHalf>>>,
    gmcp_store: Arc<Mutex<GMCPStore>>,
) {
    match event {
        TelnetEvents::DataReceive(data) => {
            debug!("DataReceive event: {} bytes", data.len());
            let data_vec = data.to_vec();
            let lines = parse_ansi_codes(data_vec);
            for line in lines {
                let full_text: String = line.iter().map(|span| span.content.clone()).collect();
                debug!("Received line: {}", full_text);

                // If the line contains "comm.channel", try to parse it as GMCP.
                if full_text.to_lowercase().contains("comm.channel") {
                    debug!("GMCP candidate detected in normal text: {}", full_text);
                    if let Some(json_start) = full_text.find('{') {
                        let maybe_json = &full_text[json_start..];
                        if let Ok(cc) = serde_json::from_str::<CommChannel>(maybe_json) {
                            let parsed_msg = parse_gmcp_message(&cc.msg);
                            let mut chat_spans = vec![Span::styled(
                                format!("[{}] {}: ", cc.chan, cc.player),
                                Style::default().fg(Color::Green),
                            )];
                            chat_spans.extend(parsed_msg);
                            let _ = tx.send(TelnetMessage::ChatMessage(chat_spans)).await;
                            continue;
                        }
                    }
                }
                let _ = tx.send(TelnetMessage::MUDOutput(line)).await;
            }
        }
        TelnetEvents::Subnegotiation(subneg) => {
            debug!("Received Subnegotiation: option={}, buffer={:?}", subneg.option, subneg.buffer);
            if subneg.option == TELOPT_GMCP {
                let gmcp_str = String::from_utf8_lossy(&subneg.buffer).to_string();
                debug!("Received GMCP subnegotiation: {}", gmcp_str);
                if let Some((package, value)) = parse_gmcp(&gmcp_str) {
                    {
                        let mut store = gmcp_store.lock().await;
                        store.update(&package, value.clone());
                    }
                    debug!("Updated GMCP store with package: {}", package);
                    if let Some(msg) = parse_known_gmcp_modules(&gmcp_str) {
                        let _ = tx.send(msg).await;
                        return;
                    }
                } else {
                    debug!("Unable to parse GMCP message: {}", gmcp_str);
                }
            } else {
                debug!("Received non-GMCP subnegotiation: option={}, buffer={:?}", subneg.option, subneg.buffer);
            }
        }
        TelnetEvents::DataSend(nego_bytes) => {
            let data_vec = nego_bytes.to_vec();
            let mut wh = write_half_arc.lock().await;
            if let Some(ref mut owned_wh) = *wh {
                if let Err(e) = owned_wh.write_all(&data_vec).await {
                    error!("Telnet negotiation write error: {}", e);
                }
            }
        }
        TelnetEvents::IAC(iac) => {
            debug!("Received IAC command: {:?}", iac);
        }
        _ => {
            debug!("Unhandled Telnet event: {:?}", event);
        }
    }
}
