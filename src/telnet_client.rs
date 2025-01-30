// src/telnet_client.rs

use crate::ansi_color::COLOR_MAP;
use libmudtelnet::events::TelnetEvents;
use libmudtelnet::Parser;
use ratatui::style::Color;
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender;
use log::{info, error};

#[derive(Debug, Deserialize)]
pub struct CommChannel {
    chan: String,
    msg: String,
    player: String,
}

pub enum TelnetMessage {
    MUDOutput(Vec<(String, Color)>),
    ChatMessage(String),
    Disconnect,
}

pub struct TelnetClient {
    stream: Option<TcpStream>,
    parser: Parser,
    incomplete_sequence: Vec<u8>,
    sender: Sender<TelnetMessage>,
}

impl TelnetClient {
    pub fn new(sender: Sender<TelnetMessage>) -> Self {
        Self {
            stream: None,
            parser: Parser::new(),
            incomplete_sequence: Vec::new(),
            sender,
        }
    }

    pub async fn connect(&mut self, ip_address: &str, port: &str) -> Result<(), String> {
        let addr = format!("{}:{}", ip_address, port);
        let stream = TcpStream::connect(addr)
            .await
            .map_err(|e| format!("Connection failed: {}", e))?;
        self.stream = Some(stream);
        self.send_command("telnet gmcp Core.Hello JSON\r\n").await?;
        Ok(())
    }

    pub async fn run(&mut self) {
        let mut read_buffer = [0; 8192];
        loop {
            if let Some(ref mut stream) = self.stream {
                match stream.read(&mut read_buffer).await {
                    Ok(0) => {
                        let _ = self.sender.send(TelnetMessage::Disconnect).await;
                        break;
                    }
                    Ok(n) => {
                        let mut data = self.incomplete_sequence.clone();
                        data.extend_from_slice(&read_buffer[..n]);
                        self.incomplete_sequence = data.clone();
                        
                        let events = self.parser.receive(&data);
                        self.incomplete_sequence = data;

                        for event in events {
                            match event {
                                TelnetEvents::DataReceive(data) => {
                                    self.handle_data_receive(&data).await;
                                }
                                TelnetEvents::DataSend(data) => {
                                    self.handle_data_send(&data).await;
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => {
                        error!("Read error: {}", e);
                        let _ = self.sender.send(TelnetMessage::Disconnect).await;
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    async fn handle_data_receive(&self, data: &[u8]) {
        // Log raw data for debugging
        if let Ok(text) = std::str::from_utf8(data) {
            info!("Received data: {}", text);
        } else {
            info!("Received non-UTF8 data");
        }
    
        let parsed = parse_ansi_codes(data.to_vec());
        for line in parsed {
            if let Err(e) = self.sender.send(TelnetMessage::MUDOutput(line)).await {
                error!("Failed to send MUD output: {}", e);
            }
        }
    
        if let Ok(text) = std::str::from_utf8(data) {
            if text.contains("comm.channel") {
                if let Some(json_start) = text.find('{') {
                    let json_str = &text[json_start..];
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
                        if let Some(comm) = value.get("comm.channel") {
                            if let Ok(comm_channel) = serde_json::from_value::<CommChannel>(comm.clone()) {
                                let msg = format!("{} tells the group '{}'\n", comm_channel.player, comm_channel.msg);
                                let _ = self.sender.send(TelnetMessage::ChatMessage(msg)).await;
                            }
                        }
                    }
                }
            }
        }
    }
    

    async fn handle_data_send(&mut self, data: &[u8]) {
        if let Some(ref mut stream) = self.stream {
            if let Err(e) = stream.write_all(data).await {
                error!("Write error: {}", e);
            }
        }
    }

    pub async fn send_command(&mut self, command: &str) -> Result<(), String> {
        if let Some(ref mut stream) = self.stream {
            // Ensure the command ends with CRLF
            let formatted_command = if command.ends_with("\r\n") {
                command.to_string()
            } else {
                format!("{}\r\n", command)
            };
            
            // Log the command being sent for debugging
            info!("Sending command: {}", formatted_command.trim_end());
    
            stream.write_all(formatted_command.as_bytes())
                .await
                .map_err(|e| format!("Write failed: {}", e))
        } else {
            error!("Attempted to send command while not connected");
            Err("Not connected".to_string())
        }
    }
    
}

pub enum AnsiState {
    Normal,
    Escaped,
    Parsing(Vec<u8>),
}

pub fn parse_ansi_codes(buffer: Vec<u8>) -> Vec<Vec<(String, Color)>> {
    let mut results = Vec::new();
    let mut current_line = Vec::new();
    let mut current_text = String::new();
    let mut current_color = Color::White;
    let mut state = AnsiState::Normal;

    for byte in buffer {
        match state {
            AnsiState::Normal => {
                if byte == 0x1B {
                    state = AnsiState::Escaped;
                    if !current_text.is_empty() {
                        current_line.push((current_text.clone(), current_color));
                        current_text.clear();
                    }
                } else {
                    current_text.push(byte as char);
                }
            }
            AnsiState::Escaped => {
                if byte == b'[' {
                    state = AnsiState::Parsing(Vec::new());
                } else {
                    state = AnsiState::Normal;
                }
            }
            AnsiState::Parsing(ref mut buf) => {
                if byte == b'm' {
                    let codes = String::from_utf8_lossy(buf).to_string();
                    for code in codes.split(';') {
                        match code {
                            "0" => current_color = Color::White,
                            "1" => {
                                // Handle bold or bright colors if needed
                                // For simplicity, you can map it to bright variants
                            }
                            // Handle extended 256-color codes
                            code if code.starts_with("38;5;") => {
                                if let Some(color_code) = code.split(';').nth(2) {
                                    if let Some(&color) = COLOR_MAP.get(color_code) {
                                        current_color = color;
                                    }
                                }
                            }
                            // Handle standard color codes
                            _ => {
                                if let Some(new_color) = COLOR_MAP.get(code) {
                                    current_color = *new_color;
                                }
                            }
                        }
                    }
                    buf.clear();
                    state = AnsiState::Normal;
                } else if byte.is_ascii_digit() || byte == b';' {
                    buf.push(byte);
                } else {
                    state = AnsiState::Normal;
                }
            }
        }
    }

    if !current_text.is_empty() {
        current_line.push((current_text, current_color));
    }

    if !current_line.is_empty() {
        results.push(current_line);
    }

    results
}
