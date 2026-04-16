# Hydra Co-op Launcher

> Maintained by [DrLegitamate](https://github.com/DrLegitamate)

---

## What Is Hydra Co-op?

Hydra Co-op lets two or more people play the **same game at the same time on one Linux PC** — each with their own keyboard, mouse, or controller.

It does this by:

- Opening multiple copies of the game side-by-side on your screen
- Routing each player's controller or keyboard to their own copy of the game
- Connecting the copies over a fake local network so they can see each other

> **Do I need to know how to code?** No. Once it's installed, you just open the app, pick your game, and click Launch.

---

## What You Need Before You Start

| Requirement | Why | How to get it |
|---|---|---|
| A Linux PC | The app only runs on Linux | — |
| The game you want to play (Linux or Windows version) | Obviously | Steam, itch.io, etc. |
| One controller/keyboard per player | Each player needs their own input device | USB or Bluetooth |
| Steam + Proton (Windows games only) | Lets Linux run Windows games | Install [Steam](https://store.steampowered.com/about/), then inside Steam go to **Settings → Compatibility → Enable Steam Play** |

---

## Step 1 — Install the Build Tools (one time only)

Open a terminal and run these commands one at a time.

**Ubuntu / Debian / Linux Mint:**
```bash
sudo apt update
sudo apt install -y curl build-essential libgtk-4-dev libevdev-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

**Fedora / RHEL:**
```bash
sudo dnf install -y curl gcc gtk4-devel libevdev-devel
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

**Arch Linux / Manjaro:**
```bash
sudo pacman -S --needed curl base-devel gtk4 libevdev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

---

## Step 2 — Download Hydra Co-op

```bash
git clone https://github.com/DrLegitamate/Hydra-Co-op.git
cd Hydra-Co-op
```

If you don't have `git`, install it first:
- Ubuntu/Debian: `sudo apt install git`
- Fedora: `sudo dnf install git`
- Arch: `sudo pacman -S git`

---

## Step 3 — Build the App

```bash
cargo build --release
```

This takes a few minutes the first time (it downloads and compiles everything). When it finishes you'll see a line like `Finished release`.

The program file is at `./target/release/hydra-coop-launcher`.

---

## Step 4 — Give the App Permission to See Your Controllers

This is a one-time setup so the app can read and route input from your keyboards, mice, and controllers.

```bash
# Create the uinput group if it doesn't exist yet
sudo groupadd uinput 2>/dev/null || true

# Add yourself to the input and uinput groups
sudo usermod -aG input,uinput "$USER"

# Create the udev rule that grants group access to /dev/uinput
echo 'KERNEL=="uinput", MODE="0660", GROUP="uinput"' | sudo tee /etc/udev/rules.d/99-uinput.rules

# Reload the rule
sudo udevadm control --reload-rules && sudo udevadm trigger

# Load the uinput kernel module
sudo modprobe uinput

# Make uinput load automatically after reboot
echo 'uinput' | sudo tee /etc/modules-load.d/uinput.conf
```

**Log out and log back in** after running these commands. The group change only takes effect after a fresh login.

---

## Step 5 — Launch the App

**Graphical interface (easiest):**
```bash
./target/release/hydra-coop-launcher
```

A window opens. Here's what to do:

1. **Number of Players** — choose 2, 3, or 4 (up to 8).
2. **Game Executable** — click Browse and find your game's `.exe` (Windows games) or Linux binary.
3. **Layout** — choose how the windows are arranged:
   - *Horizontal* — windows sit side by side (best for widescreen monitors)
   - *Vertical* — windows stack on top of each other
4. **Use Proton** — turn this on for Windows `.exe` games.
5. **Input Devices** — pick which controller or keyboard each player uses from the drop-down menus.
6. Click **Launch**.

The game opens in multiple windows, each controlled by a different player. Press **Ctrl+C** in the terminal (or close the app window) to stop everything cleanly.

---

## Using the Command Line Instead

If you prefer the terminal, here's the pattern:

```bash
./target/release/hydra-coop-launcher \
    --game-executable "/path/to/your/game" \
    --instances 2 \
    --input-devices "Auto-detect" \
    --input-devices "Auto-detect" \
    --layout horizontal
```

For a Windows game:
```bash
./target/release/hydra-coop-launcher \
    --game-executable "/path/to/YourGame.exe" \
    --instances 2 \
    --input-devices "Auto-detect" \
    --input-devices "Auto-detect" \
    --layout horizontal \
    --proton
```

### All options

| Option | What it does | Example |
|---|---|---|
| `--game-executable` | Path to the game file | `--game-executable "/home/user/games/mygame"` |
| `--instances` | How many copies to open (1–8) | `--instances 2` |
| `--input-devices` | Which device each player uses (repeat once per player) | `--input-devices "Auto-detect"` |
| `--layout` | Window arrangement: `horizontal`, `vertical` | `--layout horizontal` |
| `--proton` | Use Proton for Windows games | `--proton` |
| `--debug` | Show detailed log output for troubleshooting | `--debug` |
| `--config` | Load settings from a specific file | `--config ~/my-game-profile.toml` |

---

## Saving Settings (Config File)

You can save your settings in a file so you don't have to type them every time.

The default location is `~/.config/hydra-coop/config.toml`. Create it with any text editor:

```toml
# Path to the game
game_paths = ["/home/yourname/games/mygame/mygame.exe"]

# Input device for each player ("Auto-detect" picks the next available device)
input_mappings = [
    "Auto-detect",
    "Auto-detect",
]

# How to arrange the windows
window_layout = "horizontal"

# Network ports the game uses to communicate between copies
# (leave as-is if you're not sure)
network_ports = [7777, 7778]

# Set to true if the game is a Windows .exe
use_proton = false
```

Load a specific config file:
```bash
./target/release/hydra-coop-launcher --config "/home/yourname/.config/hydra-coop/mygame.toml"
```

---

## Window Layouts

### Horizontal (side by side)
```
+----------+----------+
|          |          |
| Player 1 | Player 2 |
|          |          |
+----------+----------+
```

### Vertical (stacked)
```
+--------------------+
|      Player 1      |
+--------------------+
|      Player 2      |
+--------------------+
```

---

## Finding Your Controller / Keyboard Name

If "Auto-detect" doesn't work, you can pick a specific device.

Run this to list all connected input devices:
```bash
ls /dev/input/by-id/
```

You'll see names like `usb-Logitech_Gamepad_F310-event-joystick`. Use that full name with `--input-devices`.

To see what events a device sends (press Ctrl+C to stop):
```bash
sudo evtest /dev/input/event0
```
(Try different numbers — `event0`, `event1`, etc. — until you find your device.)

---

## Playing Windows Games (Proton)

1. Install Steam on your system.
2. Inside Steam: go to **Steam → Settings → Compatibility** and turn on **"Enable Steam Play for all other titles"**.
3. Install at least one version of Proton from the Steam Tools library (search for "Proton" in your Steam Library).
4. Enable **Use Proton** in the Hydra Co-op GUI, or add `--proton` to the command line.

Hydra Co-op will automatically find your Proton installation. If it can't find it, tell it where Proton is:
```bash
PROTON_PATH="/home/yourname/.steam/steam/steamapps/common/Proton 9.0/proton" \
    ./target/release/hydra-coop-launcher --game-executable "/path/to/Game.exe" --instances 2 --proton
```

---

## Saving Logs to a File

Useful when you need to report a bug or figure out what went wrong:
```bash
LOG_PATH="/tmp/hydra.log" ./target/release/hydra-coop-launcher
```

The log is written to stdout **and** to the file at the same time.

---

## Troubleshooting

### "Permission denied" errors

The app can't see your input devices. Make sure you:
1. Ran the Step 4 commands above.
2. **Logged out and back in** after running them.
3. The uinput module is loaded: `sudo modprobe uinput`

### The game doesn't launch

- Check the path is correct and points to an actual file.
- If it's a Windows game, make sure **Use Proton** is turned on.
- Run with `--debug` to see detailed output: `./target/release/hydra-coop-launcher --debug ...`

### Windows game won't start with Proton

- Make sure you've installed a Proton version inside Steam (Library → Tools, search "Proton").
- Try setting the path manually: `PROTON_PATH="..." ./target/release/hydra-coop-launcher --proton ...`
- Add `PROTON_LOG=1` to see detailed Proton output.

### Windows are not arranged side by side

- The window manager waits a few seconds for game windows to appear. Slow-loading games may need a moment.
- Some games draw their own window decorations that prevent automatic resizing.

### Controllers not working in-game

- Make sure each player's controller is plugged in before launching.
- Check that the uinput module is loaded: `lsmod | grep uinput`
- Try running with `--debug` to see which devices were detected.

### The two game copies can't see each other on the network

- Check that the `network_ports` in your config match the ports the game uses for multiplayer.
- Try `--debug` mode to see the network relay output.

### Get more detail on any problem

```bash
# Show everything the app is doing
RUST_LOG=debug ./target/release/hydra-coop-launcher [your other options]

# Or use the flag
./target/release/hydra-coop-launcher --debug [your other options]

# Save to a file for reporting
LOG_PATH="/tmp/hydra-debug.log" RUST_LOG=debug ./target/release/hydra-coop-launcher [your other options]
```

---

## Frequently Asked Questions

**Does this work with every game?**
Most games work out of the box. Games with aggressive anti-cheat software (like Easy Anti-Cheat or BattlEye) will likely not work because those systems block multiple instances.

**Do both players need separate accounts/saves?**
Hydra Co-op creates separate "prefixes" for each instance, so saves should be independent. Some games share save files in fixed locations regardless — check the game's documentation.

**Can I use two keyboards on one PC?**
Yes. Linux can distinguish between multiple keyboards connected at the same time.

**My monitor is not wide enough for two windows — what do I do?**
Use the *Vertical* layout to stack windows top-to-bottom, or try a lower in-game resolution so both windows fit side by side.

**Does this use the internet?**
No. The "network emulation" creates a fake local network entirely inside your PC. No internet connection is needed or used.

---

## Contributing

Bug reports, feature requests, and pull requests are welcome on [GitHub](https://github.com/DrLegitamate/Hydra-Co-op/issues).

To work on the code:
```bash
cargo test          # run tests
cargo fmt           # format code
cargo clippy        # check for common mistakes
cargo build         # debug build
cargo build --release  # optimised build
```

---

## License

MIT — see [LICENSE](LICENSE).

## Maintainer

[DrLegitamate](https://github.com/DrLegitamate)
