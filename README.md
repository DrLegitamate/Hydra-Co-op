# Hydra Co-op Launcher

[![GitHub license](https://img.shields.io/github/license/DrLegitamate/Hydra-Co-op.svg)](https://github.com/DrLegitamate/Hydra-Co-op/blob/main/LICENSE)
[![Rust Build](https://github.com/DrLegitamate/Hydra-Co-op/actions/workflows/rust.yml/badge.svg)](https://github.com/DrLegitamate/Hydra-Co-op/actions/workflows/rust.yml)

Project maintained by [DrLegitamate](https://github.com/DrLegitamate).

Welcome to Hydra Co-op! 🎮🤝

Hydra Co-op is a tool designed for Linux to simplify setting up local split-screen co-operative gameplay by launching and managing multiple instances of a single game. It handles routing input from multiple physical devices to dedicated virtual devices, arranging game windows, emulating UDP network traffic between instances, and supports launching Windows games via Proton.

Run multiple game instances, manage inputs, and create a virtual network—all in one place! Perfect for local multiplayer setups, streaming, or testing networked games where instances need to run simultaneously on the same machine.

## Features 🌟
- 🖥️ Launch multiple instances of a single game executable simultaneously.
- 🌐 **UDP Network Emulation:** Route UDP network packets between game instances communicating on `localhost` using configurable ports.
- ⌨️ **Input Routing:** Route input from dedicated physical devices (keyboards, mice, gamepads) to individual virtual input devices, one virtual device per game instance.
- 🪟 **Automatic Window Management:** Automatically resize and position game instance windows on your display according to selected layouts (Horizontal, Vertical).
- 🍷 Proton integration for launching Windows games on Linux.
- 📝 Customizable via a `config.toml` file and command-line arguments.
- 🖱️ **Graphical User Interface (GUI):** Easy-to-use visual interface for configuration and launch (default mode if no CLI arguments are provided).
- 📋 **Command-Line Interface (CLI):** Scriptable interface for launching with specified parameters.

## Requirements

* **Linux Operating System:** Designed for Linux environments.
* **Rust and Cargo:** Needed to build the project. Install via [rustup.rs](https://rustup.rs/).
* **GTK 4 Development Libraries:** Required for the Graphical User Interface. Install via your distribution's package manager (e.g., `libgtk-4-dev` on Debian/Ubuntu, `gtk4-devel` on Fedora, `gtk4` on Arch Linux).
* **libevdev:** Library for handling Linux input devices. Install via your distribution's package manager (e.g., `libevdev-dev` on Debian/Ubuntu, `libevdev-devel` on Fedora, `libevdev` on Arch Linux).
* **uinput Kernel Module:** The `uinput` kernel module must be loaded (`sudo modprobe uinput`). To load it automatically on boot, add `uinput` to `/etc/modules` or a file in `/etc/modules-load.d/`.
* **Permissions:** Your user needs **read access** to physical input devices (typically `/dev/input/event*` files) and **write access** to `/dev/uinput` to create virtual devices. The recommended way is to add your user to the `input` and `uinput` groups (you may need to create the `uinput` group if it doesn't exist) and configure udev rules for persistent permissions. Running with `sudo` is a temporary workaround but less secure.
* **Proton:** Required for launching Windows games. Proton is typically installed via Steam.

## Installation

1.  **Clone the Repository:**
    ```bash
    git clone [https://github.com/DrLegitamate/Hydra-Co-op.git](https://github.com/DrLegitamate/Hydra-Co-op.git)
    cd Hydra-Co-op
    ```
    *(Replace `https://github.com/DrLegitamate/Hydra-Co-op.git` with the actual repository URL if it's different)*
2.  **Install Rust and Cargo:** If you don't have Rust installed, follow the instructions on [rustup.rs](https://rustup.rs/).
3.  **Install Dependencies:** Use your distribution's package manager to install GTK 4 and libevdev development libraries as listed in the Requirements section.
4.  **Build the Project:**
    ```bash
    cargo build --release
    ```
    The executable will be located at `./target/release/hydra-coop-launcher`.
5.  **Set up Permissions:** Add your user to the `input` and `uinput` groups:
    ```bash
    sudo groupadd uinput # If group does not exist
    sudo usermod -aG input,uinput $USER
    # Log out and log back in for group changes to take effect.
    ```
    You might also need to add a udev rule to ensure `/dev/uinput` has appropriate permissions for the `uinput` group persistently. Create a file like `/etc/udev/rules.d/99-uinput.rules` with content similar to `KERNEL=="uinput", MODE="0660", GROUP="uinput"`.
6.  **Load uinput Module:** Ensure the kernel module is loaded as mentioned in the Requirements section.

## Usage

Hydra Co-op Launcher can be run in either CLI or GUI mode.

* **Default Behavior:** If you run the executable without providing the required CLI arguments (`--game-executable`, `--instances`, `--input-devices`, `--layout`), it will launch the GUI.
* **Explicit GUI:** You can force the GUI using the `--gui` flag.
* **CLI Mode:** Provide all the required CLI arguments to bypass the GUI and launch directly.

### Graphical User Interface (GUI)

Launch the GUI:

```bash
./target/release/hydra-coop-launcher
# Or
./target/release/hydra-coop-launcher --gui
The GUI provides fields and controls for configuring your launch:

Number of Players: Select the number of game instances to launch (and players).
Profile Name: (Optional, currently for identification) A name for this configuration profile (future use for managing multiple saved configs).
Select Game Executable: Browse and select the game executable file.
Split-Screen Layout: Choose how windows are arranged (Horizontal, Vertical, Custom - Note: Custom layout configuration is a future feature and the current "custom" option behaves like a default/unmanaged layout).
Use Proton: Check this box to launch a Windows executable using Proton.
Input Assignments: Dynamically generated dropdowns for each player to select their physical input device. Choose "Auto-detect" or a specific device name from the list of available devices.
Save Settings: Saves the current GUI configuration to config.toml.
Launch Game: Starts the game instances with the selected settings.
Cancel: Closes the GUI window.
Command-Line Interface (CLI)
To use the CLI, you must provide all of the following required arguments:

Bash

./target/release/hydra-coop-launcher \
    --game-executable <FILE> \
    --instances <NUMBER> \
    --input-devices <DEVICE_NAME_FOR_PLAYER1> \
    [--input-devices <DEVICE_NAME_FOR_PLAYER2>] \
    # ... add --input-devices for each player up to <NUMBER> ... \
    --layout <LAYOUT> \
    [--proton] \
    [--debug]
Arguments:

--game-executable <FILE>: Required in CLI mode. Path to the game executable.
--instances <NUMBER>: Required in CLI mode. Number of game instances to launch.
--input-devices <DEVICE_NAME>: Required in CLI mode. Specify a physical input device name for a specific player instance. Provide one --input-devices argument per player instance, in order (first flag for player 1, second for player 2, etc.).
Device Names: Find names using ls /dev/input/by-id/ (recommended for stable names) or evtest /dev/input/eventX.
Use "Auto-detect" (in quotes) to let the launcher automatically assign the next available physical input device.
If fewer --input-devices are provided than instances, the remaining instances will have no input device assigned.
--layout <LAYOUT>: Required in CLI mode. Window layout style. Choose horizontal, vertical, or custom.
--proton: Optional flag. Use Proton to launch the game executable.
--debug: Optional flag. Enable debug logging (sets RUST_LOG=debug).
Example CLI Usage (2 Players, Horizontal Split, 1 Specific Device, 1 Auto-detect):

Bash

# Assuming you have a gamepad with a stable name like "usb-Logitech_Gamepad_F310-event-joystick"
./target/release/hydra-coop-launcher \
    --game-executable "/path/to/my/game.exe" \
    --instances 2 \
    --input-devices "usb-Logitech_Gamepad_F310-event-joystick" \
    --input-devices "Auto-detect" \
    --layout horizontal \
    --proton # If launching a Windows game
Configuration File (config.toml)
The launcher loads settings from config.toml at startup and can save settings from the GUI. By default, it looks for config.toml in the current working directory.

You can specify a different configuration file path using the CONFIG_PATH environment variable:

Bash

CONFIG_PATH="/home/user/.config/hydra/game_profile.toml" ./target/release/hydra-coop-launcher
The config.toml file uses the TOML format. Here's an example structure reflecting the fields used by the code:

Ini, TOML

# Example config.toml

game_paths = [
    # The path to the game executable.
    # Only the *first* path in this list is currently used when launching multiple instances.
    "/path/to/your/game/executable"
]

input_mappings = [
    # List of input assignments for each player instance (0-indexed).
    # Each item is a string:
    # - "Auto-detect" to automatically assign an available device.
    # - A string representing the unique identifier of a physical input device.
    #   The GUI saves a serialized JSON string of the DeviceIdentifier here.
    #   When editing manually for CLI, use the device name (e.g., from /dev/input/by-id/).
    "Auto-detect",
    "Auto-detect",
    # ... add one entry for each desired player instance ...
]

window_layout = "horizontal" # Or "vertical", "custom"

network_ports = [
    # List of UDP ports that game instances use for communication on localhost.
    # The network emulator uses these ports to set up mappings.
    # The index in this list often corresponds to the instance index (e.g., port 0 for instance 0, port 1 for instance 1, etc.).
    7777,
    7778,
    # ... add one port for each instance's primary communication port ...
]

use_proton = false # Set to true to use Proton for launching
Settings provided via command-line arguments will override settings loaded from the configuration file.

Window Layouts 🖼️
The window manager automatically arranges game windows.

Horizontal: Splits the screen horizontally, stacking windows side-by-side.
Plaintext

# Horizontal (2 Players)
+-------+-------+
|       |       |
| Inst0 | Inst1 |
|       |       |
+-------+-------+
Vertical: Splits the screen vertically, stacking windows top-to-bottom.
Plaintext

# Vertical (2 Players)
+---------------+
|    Inst0    |
+---------------+
|    Inst1    |
+---------------+
Custom: (Future feature) Allows defining a specific layout configuration. Currently, selecting "Custom" does not apply a specific predefined layout; windows may appear in their default positions.
(You can add screenshots of the layouts here later)

Network Emulation
The built-in UDP network emulator is designed for games that expect to communicate with other instances running on the same machine (localhost) using UDP on specific, known ports.

You must configure the network_ports in your config.toml with the UDP ports that your target game instances use for inter-instance communication. The emulator will then attempt to set up internal routing to redirect packets between the game instances based on these ports.

Note: This network emulation is currently a basic UDP forwarding mechanism on localhost based on destination ports derived from the network_ports config and the emulator's bound ports. It may not work out-of-the-box for all games, especially those using TCP, broadcasting/multicasting, complex network discovery, or different communication patterns. The network mapping logic is illustrative and game-specific adjustments might be needed.

Proton Support 🍷
If you are launching a Windows game executable (.exe), check the "Use Proton" box in the GUI or use the --proton flag in the CLI. The launcher will attempt to run the specified executable using Proton.

Ensure Proton is installed and accessible on your system. The launcher looks for proton in common system locations. You can also specify the path to the Proton executable using the PROTON_PATH environment variable.

Bash

PROTON_PATH="/path/to/your/Proton/dist/bin/proton" ./target/release/hydra-coop-launcher --game-executable "/path/to/WindowsGame.exe" --instances 2 --proton ...
Input Management 🕹️
To map specific physical input devices (like your keyboard, mouse, or gamepads) to game instances, you need to know their names or stable identifiers.

You can find the names and identifiers of your input devices using:

ls /dev/input/by-id/: This directory often contains symbolic links named based on device vendor, product, and serial number (e.g., usb-Logitech_USB_Optical_Mouse-event-mouse). These names are stable across reboots and recommended for use.
evtest /dev/input/eventX: Run evtest on an event device file (e.g., /dev/input/event0, /dev/input/event1, etc. - try them until you find your device) and look for the "Device name:" line in the output. The /dev/input/eventX paths themselves can change between boots, so using names/IDs from by-id is preferred.
Use these names with the --input-devices CLI argument or select them from the dropdown menus in the GUI.

Choosing "Auto-detect" in the CLI or "Auto-detect" in the GUI dropdown will assign the first available physical input device that hasn't already been explicitly assigned or auto-detected for another instance.

Troubleshooting 🔧
Common Issues:
Permission Errors: If you encounter "Permission denied" errors related to accessing /dev/input/event* or /dev/uinput, ensure your user is in the input and uinput groups and that the uinput kernel module is loaded. Setting up persistent udev rules for /dev/uinput is recommended. Running with sudo might work but is not a secure long-term solution.
Proton Games Not Launching: If using --proton or checking the GUI box and launch fails, ensure Proton is installed and the proton executable is accessible. You can try specifying its path via the PROTON_PATH environment variable. Set PROTON_LOG=1 in the environment before running the launcher (PROTON_LOG=1 ./target/release/hydra-coop-launcher ...) for detailed Proton debug logs in ~/steam-<appid>/proton-<version>/.
Game Windows Not Arranged: The window manager attempts to find game windows associated with the launched process IDs. Some games may take time to create their windows, or their window properties might not be easily detectable by the window manager. Ensure the game windows are fully loaded and visible. The window manager includes a basic retry mechanism.
No Input in Game Instances: If input is not working in the game instances, ensure you have selected the correct input devices (or used "Auto-detect") and that the uinput kernel module is loaded with correct permissions for your user to write to /dev/uinput. Check the logs for input-related errors. Verify using evtest that your physical device is generating events.
Network Communication Issues: If game instances cannot communicate with each other, double-check that the network_ports in your config.toml accurately match the UDP ports the game instances use for communication on localhost. Verify that the network emulator started successfully in the logs. Check firewall settings if applicable (e.g., sudo ufw allow 8080:8090/udp for UDP ports 8080-8090).
Enable Debug Logging:
For verbose output that can help diagnose issues, set the RUST_LOG environment variable:

Bash

RUST_LOG=debug ./target/release/hydra-coop-launcher [options...]
# Or use the --debug CLI flag:
./target/release/hydra-coop-launcher --debug [options...]
You can also specify a log file path using LOG_PATH="/path/to/your/log.txt".

Contributing 🤝
We welcome contributions! If you find bugs, have feature requests, or want to contribute code, please feel free to submit issues or pull requests on GitHub.

License1
This project is licensed under the MIT License.

Maintainer
DrLegitamate

Support
For questions or issues, please open an issue on the GitHub repository.


This README provides a comprehensive overview of your project, its features, requirements, installation, usage (both GUI and CLI), configuration, and troubleshooting. It also accurately reflects the current state of the code, including the implemented features and noted limitations or future work.

What would you like to work on next? We could continue refining the GUI, implement the more advanced network mapping logic, work on process monitoring, or any other aspect of the project. Let me know your priority!
