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
