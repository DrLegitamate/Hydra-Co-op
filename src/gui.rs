//! GTK4 front-end for the Hydra Co-op Launcher.
//!
//! The window is a single scrollable page with five sections:
//!   1. Game            — pick the executable
//!   2. Players         — number of players and per-player input devices
//!   3. Layout          — horizontal / vertical / 2x2 grid
//!   4. Options         — Proton toggle
//!   5. Log             — live status output
//!
//! "Save" writes the current choices to ~/.config/hydra-coop/config.toml.
//! "Launch" runs the core logic on a background thread and streams log
//! updates back to the UI.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use gtk::gdk;
use gtk::glib;
use gtk::pango;
use gtk::prelude::*;
use gtk::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CheckButton, ComboBoxText,
    CssProvider, FileChooserAction, FileChooserDialog, Frame, Grid, HeaderBar, Label, MessageDialog,
    MessageType, Orientation, PolicyType, ResponseType, ScrolledWindow, Separator, Spinner,
    TextBuffer, TextView, ToggleButton,
};
use log::{error, info};

use crate::adaptive_config::AdaptiveConfigManager;
use crate::config::Config;
use crate::input_mux::{DeviceIdentifier, InputAssignment};
use crate::run_core_logic;
use crate::window_manager::Layout;

/// All mutable UI state the signal handlers need.
struct GuiState {
    window: ApplicationWindow,
    available_devices: Vec<DeviceIdentifier>,
    file_path_label: Label,
    game_path: RefCell<Option<PathBuf>>,
    players_combo: ComboBoxText,
    input_rows: RefCell<Vec<ComboBoxText>>,
    input_rows_box: GtkBox,
    layout_toggle: LayoutToggle,
    proton_checkbox: CheckButton,
    launch_button: Button,
    save_button: Button,
    status_label: Label,
    status_spinner: Spinner,
    log_buffer: TextBuffer,
    adaptive_config: RefCell<Option<AdaptiveConfigManager>>,
}

/// The three layout-mode toggle buttons grouped together.
struct LayoutToggle {
    horizontal: ToggleButton,
    vertical: ToggleButton,
    grid: ToggleButton,
}

impl LayoutToggle {
    fn selected(&self) -> Layout {
        if self.vertical.is_active() {
            Layout::Vertical
        } else if self.grid.is_active() {
            Layout::Grid2x2
        } else {
            Layout::Horizontal
        }
    }

    fn set_from_str(&self, value: &str) {
        match value {
            "vertical" => self.vertical.set_active(true),
            "grid2x2" => self.grid.set_active(true),
            _ => self.horizontal.set_active(true),
        }
    }

    fn as_config_string(&self) -> &'static str {
        if self.vertical.is_active() {
            "vertical"
        } else if self.grid.is_active() {
            "grid2x2"
        } else {
            "horizontal"
        }
    }
}

pub fn run_gui(
    available_devices: Vec<DeviceIdentifier>,
    initial_config: Config,
    adaptive_config: Option<AdaptiveConfigManager>,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = Application::new(Some("com.hydra.coop.launcher"), Default::default());

    let devices = Rc::new(available_devices);
    let initial_config = Rc::new(initial_config);
    let adaptive = Rc::new(RefCell::new(adaptive_config));

    app.connect_activate(move |app| {
        load_custom_css();
        let state = build_main_window(app, &devices, &initial_config, adaptive.borrow_mut().take());
        populate_from_config(&state, &initial_config);
        wire_signals(state.clone());
        state.window.present();
    });

    app.run();
    Ok(())
}

fn load_custom_css() {
    if let Some(display) = gdk::Display::default() {
        let provider = CssProvider::new();
        provider.load_from_data(include_str!("../assets/style.css"));
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn build_main_window(
    app: &Application,
    devices: &Rc<Vec<DeviceIdentifier>>,
    initial_config: &Config,
    adaptive_config: Option<AdaptiveConfigManager>,
) -> Rc<GuiState> {
    let window = ApplicationWindow::new(app);
    window.set_title(Some("Hydra Co-op Launcher"));
    window.set_default_size(760, 680);
    window.add_css_class("main-window");

    let header = HeaderBar::new();
    header.set_title_widget(Some(&Label::builder()
        .label("Hydra Co-op Launcher")
        .css_classes(["title-label"])
        .build()));
    window.set_titlebar(Some(&header));

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.add_css_class("main-container");

    let scrolled = ScrolledWindow::new();
    scrolled.set_policy(PolicyType::Never, PolicyType::Automatic);
    scrolled.set_vexpand(true);

    let content = GtkBox::new(Orientation::Vertical, 16);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);

    // --- Game selection -----------------------------------------------------
    let (game_frame, file_path_label, browse_button) = build_game_section();
    content.append(&game_frame);

    // --- Players ------------------------------------------------------------
    let (players_frame, players_combo, input_rows_box) = build_players_section();
    content.append(&players_frame);

    // --- Layout -------------------------------------------------------------
    let (layout_frame, layout_toggle) = build_layout_section();
    content.append(&layout_frame);

    // --- Options ------------------------------------------------------------
    let (options_frame, proton_checkbox) = build_options_section();
    content.append(&options_frame);

    // --- Actions ------------------------------------------------------------
    let (action_box, save_button, launch_button) = build_action_buttons();
    content.append(&action_box);

    // --- Status + Log -------------------------------------------------------
    let (log_frame, status_label, status_spinner, log_buffer) = build_status_section();
    content.append(&log_frame);

    scrolled.set_child(Some(&content));
    root.append(&scrolled);
    window.set_child(Some(&root));

    let state = Rc::new(GuiState {
        window,
        available_devices: devices.as_ref().clone(),
        file_path_label: file_path_label.clone(),
        game_path: RefCell::new(initial_config.primary_game_path().cloned()),
        players_combo: players_combo.clone(),
        input_rows: RefCell::new(Vec::new()),
        input_rows_box,
        layout_toggle,
        proton_checkbox,
        launch_button,
        save_button,
        status_label,
        status_spinner,
        log_buffer,
        adaptive_config: RefCell::new(adaptive_config),
    });

    // Wire browse separately so we can return the Rc cleanly.
    {
        let state = Rc::clone(&state);
        browse_button.connect_clicked(move |_| on_browse_clicked(&state));
    }

    state
}

fn build_game_section() -> (Frame, Label, Button) {
    let frame = section_frame("1. Game", "Pick the game executable you want to co-op.");
    let inner = GtkBox::new(Orientation::Horizontal, 12);
    set_frame_padding(&inner);

    let browse = Button::with_label("Browse…");
    browse.add_css_class("suggested-action");
    browse.set_tooltip_text(Some("Choose the game's .exe or Linux binary"));

    let path_label = Label::new(Some("No game selected"));
    path_label.set_ellipsize(pango::EllipsizeMode::Middle);
    path_label.set_halign(Align::Start);
    path_label.set_hexpand(true);
    path_label.add_css_class("file-path-label");

    inner.append(&browse);
    inner.append(&path_label);
    frame.set_child(Some(&inner));
    (frame, path_label, browse)
}

fn build_players_section() -> (Frame, ComboBoxText, GtkBox) {
    let frame = section_frame(
        "2. Players",
        "Choose how many players and which input device each will use.",
    );

    let inner = GtkBox::new(Orientation::Vertical, 12);
    set_frame_padding(&inner);

    let header_row = GtkBox::new(Orientation::Horizontal, 12);
    let count_label = Label::new(Some("Number of players"));
    count_label.add_css_class("setting-label");
    count_label.set_halign(Align::Start);

    let combo = ComboBoxText::new();
    for i in 1..=crate::defaults::MAX_INSTANCES {
        combo.append_text(&i.to_string());
    }
    combo.set_active(Some(1));
    combo.set_tooltip_text(Some("How many copies of the game to launch"));

    header_row.append(&count_label);
    header_row.append(&combo);
    inner.append(&header_row);
    inner.append(&Separator::new(Orientation::Horizontal));

    let rows_box = GtkBox::new(Orientation::Vertical, 8);
    inner.append(&rows_box);

    frame.set_child(Some(&inner));
    (frame, combo, rows_box)
}

fn build_layout_section() -> (Frame, LayoutToggle) {
    let frame = section_frame("3. Layout", "How the game windows are arranged on screen.");

    let inner = GtkBox::new(Orientation::Horizontal, 12);
    set_frame_padding(&inner);

    let horizontal = ToggleButton::with_label("Horizontal");
    horizontal.set_active(true);
    horizontal.add_css_class("layout-radio");
    horizontal.set_tooltip_text(Some("Windows side by side (best for wide monitors)"));

    let vertical = ToggleButton::with_label("Vertical");
    vertical.add_css_class("layout-radio");
    vertical.set_tooltip_text(Some("Windows stacked top to bottom"));
    vertical.set_group(Some(&horizontal));

    let grid = ToggleButton::with_label("2×2 Grid");
    grid.add_css_class("layout-radio");
    grid.set_tooltip_text(Some("Four quadrants — use for 3–4 players"));
    grid.set_group(Some(&horizontal));

    inner.append(&horizontal);
    inner.append(&vertical);
    inner.append(&grid);
    frame.set_child(Some(&inner));

    (
        frame,
        LayoutToggle {
            horizontal,
            vertical,
            grid,
        },
    )
}

fn build_options_section() -> (Frame, CheckButton) {
    let frame = section_frame("4. Options", "Extra flags that apply to every instance.");
    let inner = GtkBox::new(Orientation::Vertical, 8);
    set_frame_padding(&inner);

    let proton = CheckButton::with_label("Use Proton (required for Windows .exe games)");
    proton.set_tooltip_text(Some(
        "Enable when launching a Windows executable. Requires Proton installed via Steam.",
    ));
    inner.append(&proton);
    frame.set_child(Some(&inner));
    (frame, proton)
}

fn build_action_buttons() -> (GtkBox, Button, Button) {
    let row = GtkBox::new(Orientation::Horizontal, 12);
    row.set_halign(Align::End);

    let save = Button::with_label("Save as defaults");
    save.add_css_class("flat");
    save.set_tooltip_text(Some("Write these settings to ~/.config/hydra-coop/config.toml"));

    let launch = Button::with_label("Launch");
    launch.add_css_class("suggested-action");
    launch.set_tooltip_text(Some("Start the game with the current settings"));
    launch.set_size_request(140, 42);

    row.append(&save);
    row.append(&launch);
    (row, save, launch)
}

fn build_status_section() -> (Frame, Label, Spinner, TextBuffer) {
    let frame = section_frame("5. Status", "Live output from the launcher.");
    let inner = GtkBox::new(Orientation::Vertical, 8);
    set_frame_padding(&inner);

    let status_row = GtkBox::new(Orientation::Horizontal, 8);
    let spinner = Spinner::new();
    let status = Label::new(Some("Ready."));
    status.set_halign(Align::Start);
    status.add_css_class("status-label");
    status_row.append(&spinner);
    status_row.append(&status);
    inner.append(&status_row);

    let log_scroll = ScrolledWindow::new();
    log_scroll.set_policy(PolicyType::Automatic, PolicyType::Automatic);
    log_scroll.set_min_content_height(160);

    let log_view = TextView::new();
    log_view.set_editable(false);
    log_view.set_cursor_visible(false);
    log_view.add_css_class("log-view");
    log_view.set_monospace(true);
    let buffer = log_view.buffer();

    log_scroll.set_child(Some(&log_view));
    inner.append(&log_scroll);
    frame.set_child(Some(&inner));
    (frame, status, spinner, buffer)
}

fn section_frame(title: &str, subtitle: &str) -> Frame {
    let frame = Frame::new(None);
    frame.add_css_class("section-frame");

    let header = GtkBox::new(Orientation::Vertical, 2);
    header.set_margin_top(8);
    header.set_margin_bottom(4);
    header.set_margin_start(12);
    header.set_margin_end(12);

    let title_label = Label::new(Some(title));
    title_label.set_halign(Align::Start);
    title_label.add_css_class("section-title");

    let subtitle_label = Label::new(Some(subtitle));
    subtitle_label.set_halign(Align::Start);
    subtitle_label.add_css_class("section-subtitle");

    header.append(&title_label);
    header.append(&subtitle_label);
    frame.set_label_widget(Some(&header));
    frame
}

fn set_frame_padding(b: &GtkBox) {
    b.set_margin_top(8);
    b.set_margin_bottom(12);
    b.set_margin_start(12);
    b.set_margin_end(12);
}

// ---------------------------------------------------------------------------
// Signal wiring
// ---------------------------------------------------------------------------

fn wire_signals(state: Rc<GuiState>) {
    {
        let combo = state.players_combo.clone();
        let state = Rc::clone(&state);
        combo.connect_changed(move |combo| {
            if let Some(text) = combo.active_text() {
                if let Ok(n) = text.parse::<usize>() {
                    rebuild_input_rows(&state, n);
                }
            }
        });
    }

    {
        let button = state.save_button.clone();
        let state = Rc::clone(&state);
        button.connect_clicked(move |_| on_save_clicked(&state));
    }

    {
        let button = state.launch_button.clone();
        let state = Rc::clone(&state);
        button.connect_clicked(move |_| on_launch_clicked(&state));
    }
}

fn on_browse_clicked(state: &Rc<GuiState>) {
    let dialog = FileChooserDialog::builder()
        .title("Select game executable")
        .action(FileChooserAction::Open)
        .modal(true)
        .transient_for(&state.window)
        .build();
    dialog.add_button("Cancel", ResponseType::Cancel);
    dialog.add_button("Open", ResponseType::Accept);

    let state = Rc::clone(state);
    dialog.connect_response(move |dialog, response| {
        if response == ResponseType::Accept {
            if let Some(file) = dialog.file() {
                if let Some(path) = file.path() {
                    state.file_path_label.set_text(&path.to_string_lossy());
                    *state.game_path.borrow_mut() = Some(path);
                }
            }
        }
        dialog.close();
    });
    dialog.show();
}

fn rebuild_input_rows(state: &Rc<GuiState>, num_players: usize) {
    // Clear existing rows.
    while let Some(child) = state.input_rows_box.first_child() {
        state.input_rows_box.remove(&child);
    }
    state.input_rows.borrow_mut().clear();

    for i in 0..num_players {
        let row = GtkBox::new(Orientation::Horizontal, 12);

        let label = Label::new(Some(&format!("Player {}", i + 1)));
        label.set_halign(Align::Start);
        label.add_css_class("player-label");
        label.set_width_chars(10);

        let combo = ComboBoxText::new();
        combo.append(Some("auto"), "Auto-detect");
        for device in state.available_devices.iter() {
            if let Ok(id) = serde_json::to_string(device) {
                combo.append(Some(&id), &device.name);
            }
        }
        combo.set_active_id(Some("auto"));
        combo.add_css_class("input-combo");
        combo.set_hexpand(true);

        row.append(&label);
        row.append(&combo);
        state.input_rows_box.append(&row);
        state.input_rows.borrow_mut().push(combo);
    }
}

fn on_save_clicked(state: &Rc<GuiState>) {
    let config = collect_config(state);
    match save_config_to_disk(&config) {
        Ok(path) => {
            append_log(state, &format!("Saved configuration to {}\n", path.display()));
            set_status(state, &format!("Saved to {}", path.display()), false);
        }
        Err(e) => {
            error!("Failed to save config: {e}");
            show_error(&state.window, "Could not save", &format!("{e}"));
        }
    }
}

fn on_launch_clicked(state: &Rc<GuiState>) {
    let Some(game_path) = state.game_path.borrow().clone() else {
        show_error(
            &state.window,
            "No game selected",
            "Click \"Browse…\" to choose a game executable first.",
        );
        return;
    };
    if !game_path.exists() {
        show_error(
            &state.window,
            "Game not found",
            &format!("The file {} does not exist.", game_path.display()),
        );
        return;
    }

    let config = collect_config(state);
    let assignments = collect_assignments(state);
    let layout = state.layout_toggle.selected();
    let use_proton = state.proton_checkbox.is_active();
    let num_players = assignments.len();

    state.launch_button.set_sensitive(false);
    state.save_button.set_sensitive(false);
    state.status_spinner.start();
    set_status(
        state,
        &format!("Launching {} player instance(s)…", num_players),
        true,
    );
    append_log(state, &format!("Launching {}\n", game_path.display()));

    let (tx, rx) = mpsc::channel::<LaunchMessage>();

    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(LaunchMessage::Log("Starting background services…\n".to_string()));
            let result = run_core_logic(
                &game_path,
                num_players,
                &assignments,
                layout,
                use_proton,
                &config,
                None,
            );
            match result {
                Ok((mut net, mut mux, mut launcher)) => {
                    let _ = tx.send(LaunchMessage::Running);
                    // Keep background services alive until all instances exit.
                    loop {
                        if !launcher.any_running() {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(500));
                    }
                    let _ = net.stop_relay();
                    let _ = mux.stop_capture();
                    launcher.shutdown_instances();
                    let _ = tx.send(LaunchMessage::Finished);
                }
                Err(e) => {
                    let _ = tx.send(LaunchMessage::Failed(format!("{e}")));
                }
            }
        });
    }

    // Poll the channel on the GTK main loop.
    let state = Rc::clone(state);
    glib::timeout_add_local(Duration::from_millis(150), move || {
        let mut finished = false;
        loop {
            match rx.try_recv() {
                Ok(LaunchMessage::Log(line)) => append_log(&state, &line),
                Ok(LaunchMessage::Running) => {
                    set_status(&state, "Game instances running. Close them to finish.", true);
                    append_log(&state, "All systems running.\n");
                }
                Ok(LaunchMessage::Finished) => {
                    set_status(&state, "Finished. Ready to launch again.", false);
                    append_log(&state, "Shutdown complete.\n");
                    finished = true;
                    break;
                }
                Ok(LaunchMessage::Failed(err)) => {
                    set_status(&state, "Launch failed.", false);
                    append_log(&state, &format!("ERROR: {err}\n"));
                    show_error(&state.window, "Launch failed", &err);
                    finished = true;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    finished = true;
                    break;
                }
            }
        }
        if finished {
            state.status_spinner.stop();
            state.launch_button.set_sensitive(true);
            state.save_button.set_sensitive(true);
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

enum LaunchMessage {
    Log(String),
    Running,
    Finished,
    Failed(String),
}

fn collect_config(state: &Rc<GuiState>) -> Config {
    let game_path = state.game_path.borrow().clone();
    let player_count = state
        .players_combo
        .active_text()
        .and_then(|t| t.parse::<usize>().ok())
        .unwrap_or(crate::defaults::MAX_INSTANCES.min(2));

    let mut input_mappings = Vec::with_capacity(player_count);
    for combo in state.input_rows.borrow().iter() {
        let value = combo
            .active_text()
            .map(|t| t.to_string())
            .unwrap_or_else(|| "Auto-detect".to_string());
        input_mappings.push(value);
    }
    while input_mappings.len() < player_count {
        input_mappings.push("Auto-detect".to_string());
    }

    let mut network_ports = Vec::with_capacity(player_count);
    for i in 0..player_count {
        network_ports.push(7777 + i as u16);
    }

    Config {
        game_paths: game_path.into_iter().collect(),
        input_mappings,
        window_layout: state.layout_toggle.as_config_string().to_string(),
        network_ports,
        use_proton: state.proton_checkbox.is_active(),
    }
}

fn collect_assignments(state: &Rc<GuiState>) -> Vec<(usize, InputAssignment)> {
    let rows = state.input_rows.borrow();
    let mut out = Vec::with_capacity(rows.len());
    for (i, combo) in rows.iter().enumerate() {
        let active_id = combo.active_id();
        let assignment = match active_id.as_deref() {
            Some("auto") | None => InputAssignment::AutoDetect,
            Some(id) => serde_json::from_str::<DeviceIdentifier>(id)
                .map(InputAssignment::Device)
                .unwrap_or(InputAssignment::AutoDetect),
        };
        out.push((i, assignment));
    }
    out
}

fn save_config_to_disk(config: &Config) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = crate::get_config_path()?;
    config.save(&path)?;
    Ok(path)
}

// ---------------------------------------------------------------------------
// Pre-fill from the loaded config
// ---------------------------------------------------------------------------

fn populate_from_config(state: &Rc<GuiState>, config: &Config) {
    if let Some(path) = config.primary_game_path() {
        state.file_path_label.set_text(&path.to_string_lossy());
        *state.game_path.borrow_mut() = Some(path.clone());
    }

    let count = config.instance_count().clamp(1, crate::defaults::MAX_INSTANCES);
    state.players_combo.set_active(Some((count - 1) as u32));

    rebuild_input_rows(state, count);
    for (combo, desired) in state
        .input_rows
        .borrow()
        .iter()
        .zip(config.input_mappings.iter())
    {
        // Try to match by device name; fall back to auto.
        if desired == "Auto-detect" {
            combo.set_active_id(Some("auto"));
        } else if let Some(dev) = state.available_devices.iter().find(|d| &d.name == desired) {
            if let Ok(id) = serde_json::to_string(dev) {
                combo.set_active_id(Some(&id));
            }
        } else {
            combo.set_active_id(Some("auto"));
        }
    }

    state.layout_toggle.set_from_str(&config.window_layout);
    state.proton_checkbox.set_active(config.use_proton);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn append_log(state: &Rc<GuiState>, text: &str) {
    let mut end = state.log_buffer.end_iter();
    state.log_buffer.insert(&mut end, text);
}

fn set_status(state: &Rc<GuiState>, text: &str, busy: bool) {
    state.status_label.set_text(text);
    if busy {
        state.status_spinner.start();
    } else {
        state.status_spinner.stop();
    }
    info!("{}", text);
}

fn show_error(parent: &ApplicationWindow, title: &str, message: &str) {
    let dialog = MessageDialog::builder()
        .transient_for(parent)
        .modal(true)
        .message_type(MessageType::Error)
        .buttons(gtk::ButtonsType::Close)
        .text(title)
        .secondary_text(message)
        .build();
    dialog.connect_response(|d, _| d.close());
    dialog.show();
}
