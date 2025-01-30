// src/ansi_color.rs

use lazy_static::lazy_static;
use std::collections::HashMap;
use ratatui::style::Color;

pub fn generate_xterm_color_map() -> HashMap<&'static str, Color> {
    let mut color_map = HashMap::new();

    // Standard colors
    color_map.insert("0;30", Color::Rgb(0, 0, 0));       // Black
    color_map.insert("0;31", Color::Rgb(128, 0, 0));     // Dark Red
    color_map.insert("0;32", Color::Rgb(0, 128, 0));     // Dark Green
    color_map.insert("0;33", Color::Rgb(128, 128, 0));   // Dark Yellow
    color_map.insert("0;34", Color::Rgb(0, 0, 128));     // Dark Blue
    color_map.insert("0;35", Color::Rgb(128, 0, 128));   // Dark Magenta
    color_map.insert("0;36", Color::Rgb(0, 128, 128));   // Dark Cyan
    color_map.insert("0;37", Color::Rgb(192, 192, 192)); // Light Gray
    color_map.insert("1;30", Color::Rgb(128, 128, 128)); // Dark Gray
    color_map.insert("1;31", Color::Rgb(255, 0, 0));     // Red
    color_map.insert("1;32", Color::Rgb(0, 255, 0));     // Green
    color_map.insert("1;33", Color::Rgb(255, 255, 0));   // Yellow
    color_map.insert("1;34", Color::Rgb(0, 0, 255));     // Blue
    color_map.insert("1;35", Color::Rgb(255, 0, 255));   // Magenta
    color_map.insert("1;36", Color::Rgb(0, 255, 255));   // Cyan
    color_map.insert("1;37", Color::Rgb(255, 255, 255)); // White

    // Generate xterm colors
    for i in 0..256 {
        let r = if i >= 16 && i < 232 {
            (((i - 16) / 36) * 51) as u8 // 0, 51, 102, ...
        } else if i >= 232 {
            (((i - 232) * 255) / 23) as u8    // Linear grayscale
        } else {
            0
        };

        let g = if i >= 16 && i < 232 {
            (((i - 16) % 36 / 6) * 51) as u8
        } else if i >= 232 { 
            (((i - 232) * 255) / 23) as u8    
        } else {
            0
        };

        let b = if i >= 16 && i < 232 {
            (((i - 16) % 6) * 51) as u8
        } else if i >= 232 { 
            (((i - 232) * 255) / 23) as u8     
        } else {
            0
        };

        let color_code = format!("38;5;{}", i);
        let color = Color::Rgb(r, g, b);
        color_map.insert(Box::leak(color_code.into_boxed_str()), color);
    }

    color_map
}

lazy_static! {
    pub static ref COLOR_MAP: HashMap<&'static str, Color> = generate_xterm_color_map();
}
