# MUD Telnet TUI Client

A terminal-based MUD (Multi-User Dungeon) client built in Rust. This project features asynchronous Telnet networking, advanced ANSI color support (for both traditional ANSI escapes and inline GMCP markers with xterm colors), GMCP handling for personal and group information (including gauges for HP, mana, movement, and enemy status), and a text-based UI built with `ratatui`.

**Note: This project is under active development.**

## Features

- **Telnet & GMCP Support**
  - Connects to MUD servers via Telnet.
  - Parses GMCP messages including personal stats (`char.vitals`, `char.maxstats`, `char.status`) and group data.
- **ANSI & Xterm Color Support**
  - Fully supports ANSI escape sequences.
  - Supports inline GMCP markers with xterm 256-color codes (e.g. `$x196`) and common color shortcuts (e.g. `$G`, `$R`, etc.).
- **User Interface**
  - Text-based UI built with `ratatui`.
  - Displays MUD output and chat messages.
  - Renders horizontal gauges for HP, Mana, and Movement above the input box.
- **Input Handling**
  - Command entry with history and autocomplete.
  - Basic navigation controls for scrolling through MUD and chat output.
- **Extensible & Future-Proof**
  - Designed to add further features as needed:
    - [ ] **Group Gauges** – Display enemy/group data (coming soon).
    - [ ] **Resizable Windows** – Clickable arrows (or key-based controls) to adjust group and chat window sizes.
    - [ ] **Full MXP Support** – Properly parse and render MXP tags.
    - [ ] **Sound Integration** – Ability to trigger sound effects for events.
    - Additional MUD client features such as scripting, macros, and more.

## Roadmap

- [x] Telnet connection and GMCP parsing  
- [x] ANSI and xterm 256-color support for both MUD output and GMCP inline markers  
- [ ] **Group Gauges** – Display detailed group member and enemy statistics  
- [ ] **Resizable Windows** – Allow dynamic resizing of the chat and group display areas  
- [ ] **Full MXP Support** – Implement parsing and rendering of MXP tags  
- [ ] **Sound Integration** – Add sound notifications and effects  
- [ ] Additional features as recommended by the community

## Installation

### Prerequisites
Make sure you have [Rust](https://www.rust-lang.org/tools/install) and Cargo installed:
```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh


### Clone the Repository
```sh
git clone https://github.com/your-repo/mud-telnet-tui.git
cd mud-telnet-tui
```

### Build the Project
```sh
cargo build --release
```

### Run the Client
```sh
cargo run --release
```

## Usage

Upon running the client, it will attempt to connect to the MUD server specified in the `main.rs` file. By default, it connects to:
```
darkwiz.org:6969
```
You can modify this in `main.rs`:
```rust
client.connect("your-mud-server.com", "port").await;
```

### Controls

--    **Command Input**:

        Type a command and press Enter to send it.
        Backspace to delete a character.
        ESC to exit the client.
--    **Output Panels**:

        MUD Output Panel – Displays game messages.
        Chat Panel – Displays chat messages.
--    **Navigation**:

        Use arrow keys and page keys for scrolling.
        (Future) Clickable arrows to adjust the size of the group and chat windows.

## Configuration

### Change MUD Connection Settings
Edit `src/main.rs`:
```rust
client.connect("your-mud-server.com", "port").await;
```

### Change ANSI Colors
Modify `src/ansi_color.rs` to update color mappings.

### Debugging & Logging
Enable logging by setting the `RUST_LOG` environment variable:
```sh
RUST_LOG=info cargo run
```

## Dependencies
This project uses:
- [`tokio`](https://crates.io/crates/tokio) - Asynchronous runtime
- [`crossterm`](https://crates.io/crates/crossterm) - Terminal input/output handling
- [`ratatui`](https://crates.io/crates/ratatui) - Terminal UI rendering
- [`serde`](https://crates.io/crates/serde) - JSON parsing
- [`libmudtelnet`](https://crates.io/crates/libmudtelnet) - Telnet protocol handling
- [`log`](https://crates.io/crates/log) - Logging

## License
This project is licensed under the MIT License.

## Contribution
Contributions are welcome! Feel free to open an issue or submit a pull request.

## Contact
For support or suggestions, contact the developer at `rpgplayers.inc@gmail.com` or open an issue on GitHub.
