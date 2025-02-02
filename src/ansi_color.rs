// src/ansi_color.rs

use ratatui::style::{Color, Style};
use ratatui::text::Span;
use std::collections::HashMap;
use lazy_static::lazy_static;

/// Build a complete color mapping.
///
/// The mapping supports:
///   - Standard SGR codes with attribute prefixes (e.g. "0;35" for dim magenta,
///     "1;35" for bright magenta)
///   - 256‑color sequences (using keys like "38;5;X")
///
/// Note: For background 256‑colors, we will later convert the code from "48;5;X" to "38;5;X".
pub fn generate_xterm_color_map() -> HashMap<&'static str, Color> {
    let mut color_map = HashMap::new();

    // Standard foreground colors (using egui values)
    color_map.insert("0;30", Color::Rgb(0, 0, 0));           // Black (dim)
    color_map.insert("0;31", Color::Rgb(128, 0, 0));         // Dark Red (dim)
    color_map.insert("0;32", Color::Rgb(0, 128, 0));         // Dark Green (dim)
    color_map.insert("0;33", Color::Rgb(128, 128, 0));       // Dark Yellow (dim)
    color_map.insert("0;34", Color::Rgb(0, 0, 128));         // Dark Blue (dim)
    color_map.insert("0;35", Color::Rgb(128, 0, 128));       // Dark Magenta (dim)
    color_map.insert("0;36", Color::Rgb(0, 128, 128));       // Dark Cyan (dim)
    color_map.insert("0;37", Color::Rgb(192, 192, 192));     // Light Gray (dim)
    color_map.insert("1;30", Color::Rgb(128, 128, 128));     // Dark Gray (bright)
    color_map.insert("1;31", Color::Rgb(255, 0, 0));         // Red (bright)
    color_map.insert("1;32", Color::Rgb(0, 255, 0));         // Green (bright)
    color_map.insert("1;33", Color::Rgb(255, 255, 0));       // Yellow (bright)
    color_map.insert("1;34", Color::Rgb(0, 0, 255));         // Blue (bright)
    color_map.insert("1;35", Color::Rgb(255, 0, 255));       // Magenta (bright)
    color_map.insert("1;36", Color::Rgb(0, 255, 255));       // Cyan (bright)
    color_map.insert("1;37", Color::Rgb(255, 255, 255));     // White (bright)

    // Build mapping for xterm 256 colors (for foreground)
    for i in 0..=255 {
        let key = format!("38;5;{}", i);
        let color = if i < 16 {
            match i {
                0 => Color::Rgb(0, 0, 0),
                1 => Color::Rgb(128, 0, 0),
                2 => Color::Rgb(0, 128, 0),
                3 => Color::Rgb(128, 128, 0),
                4 => Color::Rgb(0, 0, 128),
                5 => Color::Rgb(128, 0, 128),
                6 => Color::Rgb(0, 128, 128),
                7 => Color::Rgb(192, 192, 192),
                8 => Color::Rgb(128, 128, 128),
                9 => Color::Rgb(255, 0, 0),
                10 => Color::Rgb(0, 255, 0),
                11 => Color::Rgb(255, 255, 0),
                12 => Color::Rgb(0, 0, 255),
                13 => Color::Rgb(255, 0, 255),
                14 => Color::Rgb(0, 255, 255),
                15 => Color::Rgb(255, 255, 255),
                _ => Color::Reset,
            }
        } else if i < 232 {
            // 6×6×6 color cube.
            let index = i - 16;
            let r = (index / 36) * 51;
            let g = ((index % 36) / 6) * 51;
            let b = (index % 6) * 51;
            Color::Rgb(r as u8, g as u8, b as u8)
        } else {
            // Grayscale ramp.
            let gray = 8 + (i - 232) * 10;
            Color::Rgb(gray as u8, gray as u8, gray as u8)
        };
        let leaked_key = Box::leak(key.into_boxed_str());
        color_map.insert(leaked_key, color);
    }
    color_map
}

lazy_static! {
    pub static ref COLOR_MAP: HashMap<&'static str, Color> = generate_xterm_color_map();
}

/// Strip MXP tags from the input string.
/// For simplicity, this function removes any occurrences of <MXP> and </MXP>
/// and any other tags you choose to strip.
pub fn strip_mxp_tags(input: &str) -> String {
    input.replace("<MXP>", "").replace("</MXP>", "")
}

/// Parse ANSI escape sequences from raw bytes (converted to a UTF‑8 string)
/// into lines of styled spans. This parser preserves Unicode and supports both
/// foreground and background colors. When an SGR sequence is encountered:
/// - If the parameter string is bare (e.g. "35"), we prepend "0;" so that it is dim.
/// - If a background 256‑color sequence ("48;5;X") is encountered, we convert it
///   to a foreground lookup key ("38;5;X") for color lookup.
pub fn parse_ansi_codes(buffer: Vec<u8>) -> Vec<Vec<Span<'static>>> {
    // Convert raw bytes to a UTF‑8 string (lossy conversion preserves Unicode)
    let raw_input = String::from_utf8_lossy(&buffer);
    // First, strip MXP tags from the input.
    let input = strip_mxp_tags(&raw_input);
    let mut results = Vec::new();
    let mut current_line = Vec::new();
    let mut current_text = String::new();
    // Default state: white foreground, no background.
    let mut current_fg: Color = Color::White;
    let mut current_bg: Option<Color> = None;
    let mut current_style = Style::default().fg(current_fg);
    if let Some(bg) = current_bg {
        current_style = current_style.bg(bg);
    }

    enum State { Normal, Escaped, Parsing(String) }
    let mut state = State::Normal;
    for ch in input.chars() {
        match state {
            State::Normal => {
                if ch == '\x1B' {
                    state = State::Escaped;
                    if !current_text.is_empty() {
                        current_style = Style::default().fg(current_fg);
                        if let Some(bg) = current_bg { current_style = current_style.bg(bg); }
                        current_line.push(Span::styled(current_text.clone(), current_style));
                        current_text.clear();
                    }
                } else if ch == '\n' {
                    if !current_text.is_empty() {
                        current_style = Style::default().fg(current_fg);
                        if let Some(bg) = current_bg { current_style = current_style.bg(bg); }
                        current_line.push(Span::styled(current_text.clone(), current_style));
                        current_text.clear();
                    }
                    results.push(current_line);
                    current_line = Vec::new();
                } else if ch != '\r' {
                    current_text.push(ch);
                }
            }
            State::Escaped => {
                if ch == '[' {
                    state = State::Parsing(String::new());
                } else {
                    state = State::Normal;
                    current_text.push(ch);
                }
            }
            State::Parsing(ref mut code_str) => {
                if ch == 'm' {
                    // Finished reading an SGR sequence.
                    let code = code_str.clone();
                    log::debug!("Parsed SGR code: {}", code);
                    if code == "0" {
                        current_fg = Color::White;
                        current_bg = None;
                    } else if code.starts_with("38;5;") {
                        // 256-color foreground.
                        if let Some(&color) = COLOR_MAP.get(code.as_str()) {
                            current_fg = color;
                        } else {
                            log::debug!("Foreground key not found: {}", code);
                        }
                    } else if code.starts_with("48;5;") {
                        // 256-color background.
                        let fg_key = code.replacen("48;5;", "38;5;", 1);
                        if let Some(&color) = COLOR_MAP.get(fg_key.as_str()) {
                            current_bg = Some(color);
                        } else {
                            log::debug!("Background key not found: {}", code);
                        }
                    } else if ["40","41","42","43","44","45","46","47"].contains(&code.as_str()) {
                        // Standard background codes.
                        let bg_color = match code {
                            ref s if *s == "40" => Color::Rgb(0, 0, 0),
                            ref s if *s == "41" => Color::Rgb(128, 0, 0),
                            ref s if *s == "42" => Color::Rgb(0, 128, 0),
                            ref s if *s == "43" => Color::Rgb(128, 128, 0),
                            ref s if *s == "44" => Color::Rgb(0, 0, 128),
                            ref s if *s == "45" => Color::Rgb(128, 0, 128),
                            ref s if *s == "46" => Color::Rgb(0, 128, 128),
                            ref s if *s == "47" => Color::Rgb(192, 192, 192),
                            _ => Color::Reset,
                        };
                        current_bg = Some(bg_color);
                    } else if ["90","91","92","93","94","95","96","97"].contains(&code.as_str()) {
                        // Explicit bright foreground.
                        let key = format!("1;{}", code);
                        if let Some(&color) = COLOR_MAP.get(key.as_str()) {
                            current_fg = color;
                        }
                    } else {
                        // For standard foreground codes: if bare (no semicolon), prepend "0;".
                        let key = if code.contains(";") { code } else { format!("0;{}", code) };
                        if let Some(&color) = COLOR_MAP.get(key.as_str()) {
                            current_fg = color;
                        } else {
                            log::debug!("SGR code not found in COLOR_MAP: {}", key);
                        }
                    }
                    state = State::Normal;
                } else {
                    code_str.push(ch);
                }
            }
        }
    }
    if !current_text.is_empty() {
        current_style = Style::default().fg(current_fg);
        if let Some(bg) = current_bg { current_style = current_style.bg(bg); }
        current_line.push(Span::styled(current_text, current_style));
    }
    if !current_line.is_empty() {
        results.push(current_line);
    }
    results
}
