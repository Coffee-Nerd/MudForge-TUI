# MUD Telnet TUI Client

A terminal-based MUD (Multi-User Dungeon) client built in Rust, featuring ANSI color support, Telnet protocol handling, and a text-based UI using `ratatui`.

## Features
- Connects to a MUD server via Telnet
- Displays MUD output with ANSI color handling
- Supports input commands with a terminal interface
- Handles Telnet GMCP (Generic Mud Communication Protocol)
- Implements a UI using `ratatui`
- Supports chat message parsing and display
- Uses asynchronous Rust (`tokio`) for handling networking and input

## Installation

### Prerequisites
Ensure you have Rust and Cargo installed:
```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

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
- **Type a command** → Press `Enter` to send it
- **Backspace** → Delete a character
- **ESC** → Exit the client
- **MUD Output Panel** → Displays received messages
- **Chat Panel** → Displays chat messages from the game

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
