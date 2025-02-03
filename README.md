# Hydra Co-op

This project is maintained by [DrLegitamate](https://github.com/DrLegitamate).

Welcome to Hydra Co-op! 🎮🤝

Hydra Co-op is a tool designed to allow you to run multiple game instances simultaneously. It has features like virtual network setup, input multiplexing, and automatic window management.

Run multiple game instances, manage inputs, and create a virtual network—all in one place! Perfect for local multiplayer setups, streaming, or testing networked games.

## Features 🌟
- 🖥️ Launch multiple game instances simultaneously
- 🌐 Virtual network setup for inter-instance communication
- ⌨️ Multiplex inputs to control all instances from one device
- 🪟 Automatic window management (resize, position, decorations)
- 🍷 Proton integration for Windows games on Linux
- 📝 Customizable via config file or command-line

## Quick Start 🚀
1. **Installation**
   ```bash
   # Clone the repository
git clone https://github.com/yourusername/Hydra Co-op.git
cd Hydra Co-op

   # Build with Rust
   cargo build --release
   ```

2. **Basic Usage**
   ```bash
   # Run 3 instances of a Windows game using Proton
./target/release/Hydra Co-op \
     --game-executable "/path/to/game.exe" \
     --instances 3 \
     --input-devices /dev/input/event3 /dev/input/event4 \
     --layout horizontal \
     --proton
   ```

## Configuration ⚙️
Create `config.toml` in your project directory:
```toml
# Game settings
game_paths = [
    "/home/user/games/game1",
    "/home/user/games/game2"
]

# Network configuration (auto-assigned if empty)
network_ports = [8080, 8081, 8082]

# Window layout options: "horizontal", "vertical", or "grid"
window_layout = "horizontal"

# Input mappings (key/button to action)
input_mappings = [
    "KEY_A=JUMP",
    "KEY_ENTER=START"
]
```

## Command-Line Options 📋
| Option            | Description                                      | Example                          |
|-------------------|--------------------------------------------------|----------------------------------|
| `--game-executable` | Path to game executable (required)               | `--game-executable ./game.exe`   |
| `--instances`      | Number of instances to launch (default: 2)       | `--instances 4`                  |
| `--input-devices`  | Input device paths (find with evtest)            | `--input-devices /dev/input/event3` |
| `--layout`         | Window arrangement style                         | `--layout vertical`               |
| `--proton`         | Enable Proton for Windows games                  | (flag, no value needed)           |
| `--config`         | Custom config file path                         | `--config ./custom_config.toml`  |
| `--debug`          | Enable verbose logging                           | (flag, no value needed)           |

## Proton Support 🍷
Run Windows games on Linux seamlessly:
- Ensure Proton is installed (Steam Play compatibility tool)
- Add the `--proton` flag when launching
- Specify your Windows executable path with `--game-executable`

Example Proton launch:
```bash
./Hydra Co-op --game-executable "/path/to/WindowsGame.exe" --instances 2 --proton
```

## Input Management 🕹️
Find your input devices:
```bash
# List available input devices
evtest

# Example output:
/dev/input/event3: Logitech Gamepad
/dev/input/event4: Keyboard
```

Map devices to instances using their event numbers or paths:
```bash
# Map first controller to instance 1, second to instance 2
--input-devices event3 event4
```

## Window Layouts 🖼️
Choose from three preset layouts:
- **Horizontal Split**
  ![Horizontal Layout](path/to/horizontal_layout.png)

- **Vertical Split**
  ![Vertical Layout](path/to/vertical_layout.png)

- **Grid Layout**
  ![Grid Layout](path/to/grid_layout.png)

## Troubleshooting 🔧
### Common Issues:
- **Missing Input Devices**
  - Check device permissions: `sudo chmod a+r /dev/input/event*`
  - Verify device detection with `evtest`

- **Proton Games Not Launching**
  - Ensure Steam Proton is properly installed
  - Set `PROTON_LOG=1` for debug logs

- **Network Conflicts**
  - Use unique ports in your config file
  - Check firewall settings: `sudo ufw allow 8080:8090/tcp`

Enable Debug Logging:
```bash
RUST_LOG=debug ./Hydra Co-op [options...]
```

## Contributing 🤝
We welcome contributions! Please see our Contribution Guidelines for details.

## License
MIT

## Maintainer
Your Name (@yourusername)

## Support
Open an Issue
