# Hydra Co-op Launcher

> Project maintained by DrLegitamate

## Welcome to Hydra Co-op! 🎮🤝

Hydra Co-op is a tool designed for Linux to simplify setting up local split-screen co-operative gameplay by launching and managing multiple instances of a single game. It handles routing input from multiple physical devices to dedicated virtual devices, arranging game windows, emulating UDP network traffic between instances, and supports launching Windows games via Proton.

Run multiple game instances, manage inputs, and create a virtual network—all in one place! Perfect for local multiplayer setups, streaming, or testing networked games where instances need to run simultaneously on the same machine.

## ✨ Features

- 🎮 **Universal Game Support**: Works with ANY game without requiring game-specific handlers or configuration
- 🧠 **Adaptive Learning**: Automatically learns optimal settings for each game and improves over time
- 🔍 **Automatic Game Detection**: Detects game engines (Unity, Unreal, Godot, etc.) and applies appropriate configurations
- 🖥️ **Multiple Instances**: Launch multiple instances of a single game executable simultaneously
- 🌐 **UDP Network Emulation**: Route UDP network packets between game instances communicating on localhost using configurable ports
- ⌨️ **Input Routing**: Route input from dedicated physical devices (keyboards, mice, gamepads) to individual virtual input devices
- 🪟 **Automatic Window Management**: Resize and position game instance windows according to selected layouts
- 🍷 **Proton Integration**: Launch Windows games on Linux
- 📝 **Customizable Settings**: Configure via config.toml file or command-line arguments
- 🖱️ **Graphical User Interface**: Easy-to-use visual interface for configuration
- 📋 **Command-Line Interface**: Scriptable interface for launching with specified parameters
- 🔧 **Robust Error Handling**: Comprehensive error reporting and validation
- 📊 **Statistics and Monitoring**: Real-time status information about running services
- 🎯 **Smart Device Assignment**: Automatic and manual input device assignment options

### 🚀 Universal Game Support

Hydra Co-op now features a revolutionary **universal game support system** that eliminates the need for game-specific handlers:

- **Automatic Engine Detection**: Detects Unity, Unreal Engine, Godot, GameMaker, and other engines
- **Smart Configuration**: Automatically applies optimal settings based on detected game type
- **Adaptive Learning**: Learns from successful launches and improves configurations over time
- **Zero Configuration**: Works out-of-the-box with most games
- **Fallback Strategies**: Provides multiple approaches when games don't work with default settings

## 📋 Requirements

- **Linux Operating System**: Designed for Linux environments
- **Rust and Cargo**: Needed to build the project. Install via [rustup.rs](https://rustup.rs/)
- **GTK 4 Development Libraries**: Required for GUI
  - Debian/Ubuntu: `libgtk-4-dev`
  - Fedora: `gtk4-devel`
  - Arch Linux: `gtk4`
- **libevdev**: Library for handling Linux input devices
  - Debian/Ubuntu: `libevdev-dev`
  - Fedora: `libevdev-devel`
  - Arch Linux: `libevdev`
- **uinput Kernel Module**: Load with `sudo modprobe uinput`
  - For autoloading, add `uinput` to `/etc/modules` or create a file in `/etc/modules-load.d/`
- **Permissions**: User needs access to `/dev/input/event*` and `/dev/uinput`
- **Proton**: Required for launching Windows games (typically installed via Steam)

## 🔧 Installation

1. **Clone the Repository**:
   ```bash
   git clone https://github.com/DrLegitamate/Hydra-Co-op.git
   cd Hydra-Co-op
   ```

2. **Install Rust and Cargo** (if not already installed):
   - Follow instructions at [rustup.rs](https://rustup.rs/)

3. **Install Dependencies**:
   - Use your distribution's package manager to install GTK 4 and libevdev development libraries

4. **Build the Project**:
   ```bash
   cargo build --release
   ```
   The executable will be at `./target/release/hydra-coop-launcher`

5. **Set up Permissions**:
   ```bash
   sudo groupadd uinput  # If group does not exist
   sudo usermod -aG input,uinput $USER
   # Log out and log back in for group changes to take effect
   ```

   Create a udev rule file `/etc/udev/rules.d/99-uinput.rules`:
   ```
   KERNEL=="uinput", MODE="0660", GROUP="uinput"
   ```

6. **Load uinput Module**:
   ```bash
   sudo modprobe uinput
   ```

7. **Create Configuration Directory** (optional):
   ```bash
   mkdir -p ~/.config/hydra-coop
   ```
## 🚀 Usage

### Graphical User Interface (GUI)

Launch the GUI:
```bash
./target/release/hydra-coop-launcher
# Or
./target/release/hydra-coop-launcher --gui
```

The GUI provides:
- Number of Players selection
- Profile Name field (for future use)
- Game Executable selection
- Split-Screen Layout options
- Proton toggle for Windows games
- Input device assignment for each player
- Save and Launch buttons
- Real-time status information
- Configuration validation

### Command-Line Interface (CLI)

```bash
./target/release/hydra-coop-launcher \
    --game-executable <FILE> \
    --instances <NUMBER> \
    --input-devices <DEVICE_NAME_FOR_PLAYER1> \
    [--input-devices <DEVICE_NAME_FOR_PLAYER2>] \
    # ... add --input-devices for each player ... \
    --layout <LAYOUT> \
    [--proton] \
    [--debug]
    [--config <CONFIG_FILE>] \
    [--verbose]
```

**Arguments**:
- `--game-executable <FILE>`: Path to the game executable
- `--input-devices <DEVICE_NAME>`: Physical input device name for each player
  - Find names using `ls /dev/input/by-id/` or `evtest /dev/input/eventX`
  - Use `"Auto-detect"` to automatically assign the next available device
- `--layout <LAYOUT>`: Window layout style (`horizontal`, `vertical`, or `custom`)
- `--proton`: Use Proton to launch Windows games
- `--config <PATH>`: Path to configuration file
- `--debug`: Enable debug logging
- `--verbose`: Enable verbose output (can be used multiple times)

**Example** (2 Players, Horizontal Split):
```bash
./target/release/hydra-coop-launcher \
    --game-executable "/path/to/my/game.exe" \
    --instances 2 \
    --input-devices "usb-Logitech_Gamepad_F310-event-joystick" \
    --input-devices "Auto-detect" \
    --layout horizontal \
    --proton
```

## ⚙️ Configuration File (config.toml)

The launcher loads settings from `config.toml` at startup. By default, it looks for this file in `~/.config/hydra-coop/config.toml`.

You can specify a different configuration file:
```bash
./target/release/hydra-coop-launcher --config "/home/user/.config/hydra/game_profile.toml"
# Or using environment variable:
CONFIG_PATH="/home/user/.config/hydra/game_profile.toml" ./target/release/hydra-coop-launcher
```

**Example config.toml**:
```toml
# Path to the game executable
game_paths = [
    "/path/to/your/game/executable"
]

# Input assignments for each player instance
input_mappings = [
    "Auto-detect",
    "Auto-detect",
    # Add one entry per player
]

# Window layout: "horizontal", "vertical", or "custom"
window_layout = "horizontal" 

# UDP ports for game instances communication
network_ports = [
    7777,
    7778,
    # Add one port per instance
]

# Set to true to use Proton for launching Windows games
use_proton = false
```

Settings provided via command-line arguments override settings from the configuration file.

### Configuration Validation

The application automatically validates configuration settings:
- Ensures game executable paths exist and are executable
- Validates instance count is between 1 and 8
- Checks network ports are valid (>= 1024)
- Provides helpful error messages for invalid configurations
## 🖼️ Window Layouts

The window manager automatically arranges game windows:

### Horizontal Split
Splits the screen horizontally, stacking windows side-by-side:
```
-------+-------+
+-------+-------+
|       |       |
| Inst0 | Inst1 |
|       |       |
+-------+-------+
```

### Vertical Split
Splits the screen vertically, stacking windows top-to-bottom:
```
+---------------+
|     Inst0     |
+---------------+
|     Inst1     |
+---------------+
```

### Custom Layout
(Future feature) Currently acts as default/unmanaged layout.

## 🌐 Network Emulation

The built-in UDP network emulator is designed for games that communicate between instances on localhost using UDP.

Configure the `network_ports` in your `config.toml` with the UDP ports that your game instances use for communication. The emulator will set up internal routing to redirect packets between the game instances.

**Note**: This is a basic UDP forwarding mechanism. It may not work for all games, especially those using TCP, broadcast/multicast, or complex network discovery.

## 🍷 Proton Support

To launch Windows game executables:
- Check "Use Proton" in the GUI or use the `--proton` flag in the CLI
- Ensure Proton is installed and accessible on your system

You can specify the Proton path:
```bash
PROTON_PATH="/path/to/your/Proton/dist/bin/proton" ./target/release/hydra-coop-launcher --game-executable "/path/to/WindowsGame.exe" --instances 2 --proton ...
```

## 🕹️ Input Management

To map specific physical input devices to game instances, find their names or identifiers:

1. Using stable identifiers (recommended):
   ```bash
   ls /dev/input/by-id/
   ```

2. Using device events:
   ```bash
   evtest /dev/input/eventX  # Try different X values to find your device
   ```

Use these names with the `--input-devices` CLI argument or select them from the dropdown menus in the GUI.

### Input Assignment Options

- **Auto-detect**: Automatically assigns the next available input device
- **Specific Device**: Manually assign a specific input device by name/identifier
- **None**: No input device assigned to this instance

The application provides real-time feedback about device availability and assignment status.
## 🔧 Troubleshooting

### Common Issues

- **Permission Errors**: 
  - Ensure your user is in the `input` and `uinput` groups
  - Verify the uinput kernel module is loaded
  - Check udev rules are set up correctly

- **Proton Games Not Launching**:
  - Ensure Proton is installed
  - Try specifying its path via `PROTON_PATH` environment variable
  - Set `PROTON_LOG=1` for detailed logs: `PROTON_LOG=1 ./target/release/hydra-coop-launcher ...`

- **Window Arrangement Issues**:
  - Ensure game windows are fully loaded and visible
  - Some games may not be compatible with automatic window management

- **Input Not Working**:
  - Verify correct input devices are selected
  - Check that uinput kernel module is loaded
  - Verify permissions for /dev/uinput
  - Test physical devices with evtest

- **Network Communication Issues**:
  - Verify network_ports in config.toml match the game's UDP ports
  - Check firewall settings if applicable

- **Configuration Errors**:
  - Use `--debug` flag for detailed error information
  - Check configuration file syntax with a TOML validator
  - Ensure all paths in configuration exist and are accessible
### Enable Debug Logging

```bash
RUST_LOG=debug ./target/release/hydra-coop-launcher [options...]
# Or use the --debug flag:
./target/release/hydra-coop-launcher --debug [options...]
# For even more verbose output:
./target/release/hydra-coop-launcher --verbose --verbose [options...]
```

You can specify a log file: `LOG_PATH="/path/to/your/log.txt"`

### Getting Help

- Use `./target/release/hydra-coop-launcher --help` for command-line usage
- Check the application logs for detailed error information
- Ensure all system requirements are met
- Verify file permissions and group memberships
## 🤝 Contributing

We welcome contributions! If you find bugs, have feature requests, or want to contribute code, please submit issues or pull requests on GitHub.

### Development Setup

1. Install Rust nightly for development features
2. Run tests: `cargo test`
3. Check formatting: `cargo fmt --check`
4. Run clippy: `cargo clippy`
5. Build documentation: `cargo doc --open`
## 📄 License

This project is licensed under the MIT License.

## 👤 Maintainer

DrLegitamate

## 📞 Support

For questions or issues, please open an issue on the GitHub repository.

## 🔄 Changelog

### Version 0.1.0
- Initial release
- Basic multi-instance game launching
- Input device routing
- Window management
- Network emulation
- Proton support
- GUI and CLI interfaces
- Configuration system
- Comprehensive error handling
- Input validation and device management improvements