use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Grid, ComboBoxText, Entry, Button, Label, RadioButton, 
    FileChooserDialog, Align, Orientation, MessageDialog, DialogFlags, MessageType, ButtonsType, 
    CheckButton, Box, Frame, Separator, ScrolledWindow, TextView, TextBuffer, ProgressBar,
    Stack, StackSwitcher, HeaderBar, MenuButton, Popover, ListBox, ListBoxRow, Image,
    CssProvider, StyleContext, STYLE_PROVIDER_PRIORITY_APPLICATION
};
use crate::input_mux::{InputMux, DeviceIdentifier, InputAssignment};
use log::{info, error, warn, debug};
use std::rc::Rc;
use std::cell::RefCell;
use std::path::PathBuf;
use crate::config::{Config, ConfigError};
use crate::window_manager::Layout;
use std::collections::HashMap;
use crate::run_core_logic;
use std::thread::{self, JoinHandle};
use std::error::Error;
use std::sync::{Arc, Mutex};
use serde_json;
use crate::adaptive_config::AdaptiveConfigManager;
use std::env;

// Define a struct to hold GUI state and data accessible by signal handlers
#[derive(Default)]
struct GuiState {
    available_input_devices: Vec<DeviceIdentifier>,
    file_path_label: Option<Label>,
    num_players_combo: Option<ComboBoxText>,
    input_combos: Vec<ComboBoxText>,
    layout_radios: Vec<RadioButton>,
    profile_name_entry: Option<Entry>,
    input_fields_container: Option<Grid>,
    main_window: Option<ApplicationWindow>,
    initial_config: Config,
    use_proton_checkbox: Option<CheckButton>,
    background_services: Arc<Mutex<Option<(crate::net_emulator::NetEmulator, InputMux)>>>,
    core_logic_thread: Arc<Mutex<Option<JoinHandle<Result<(crate::net_emulator::NetEmulator, InputMux), Box<dyn StdError + Send + Sync>>>>>>,
    adaptive_config: Arc<Mutex<Option<AdaptiveConfigManager>>>,
    
    // New UI elements
    status_label: Option<Label>,
    progress_bar: Option<ProgressBar>,
    log_buffer: Option<TextBuffer>,
    launch_button: Option<Button>,
    stack: Option<Stack>,
}

/// Builds and runs the GTK application GUI with modern design
pub fn run_gui(
    available_devices: Vec<DeviceIdentifier>, 
    initial_config: Config,
    adaptive_config: Option<AdaptiveConfigManager>
) -> Result<(), Box<dyn std::error::Error>> {

    let application = Application::new(
        Some("com.hydra.coop.launcher"),
        Default::default(),
    );

    let gui_state = Rc::new(RefCell::new(GuiState::default()));
    gui_state.borrow_mut().available_input_devices = available_devices.clone();
    gui_state.borrow_mut().initial_config = initial_config.clone();

    let background_services_state = Arc::new(Mutex::new(None));
    let core_logic_thread_handle = Arc::new(Mutex::new(None));
    gui_state.borrow_mut().background_services = Arc::clone(&background_services_state);
    gui_state.borrow_mut().core_logic_thread = Arc::clone(&core_logic_thread_handle);
    gui_state.borrow_mut().adaptive_config = Arc::new(Mutex::new(adaptive_config));

    application.connect_activate(move |app| {
        // Load custom CSS for modern styling
        load_custom_css();
        
        let window = ApplicationWindow::new(app);
        window.set_title("Hydra Co-op Launcher");
        window.set_default_size(1000, 700);
        window.add_css_class("main-window");
        gui_state.borrow_mut().main_window = Some(window.clone());

        // Create header bar
        let header_bar = HeaderBar::new();
        header_bar.set_title_widget(Some(&create_title_widget()));
        header_bar.add_css_class("header-bar");
        
        // Add menu button to header
        let menu_button = create_menu_button();
        header_bar.pack_end(&menu_button);
        
        window.set_titlebar(Some(&header_bar));

        // Create main container with stack for different views
        let main_box = Box::new(Orientation::Vertical, 0);
        main_box.add_css_class("main-container");
        
        // Create stack and stack switcher
        let stack = Stack::new();
        stack.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
        stack.set_transition_duration(300);
        gui_state.borrow_mut().stack = Some(stack.clone());
        
        let stack_switcher = StackSwitcher::new();
        stack_switcher.set_stack(Some(&stack));
        stack_switcher.add_css_class("view-switcher");
        
        main_box.append(&stack_switcher);
        main_box.append(&stack);

        // Create different views
        let setup_view = create_setup_view(&gui_state, &initial_config);
        let advanced_view = create_advanced_view(&gui_state);
        let status_view = create_status_view(&gui_state);
        
        stack.add_titled(&setup_view, Some("setup"), "Game Setup");
        stack.add_titled(&advanced_view, Some("advanced"), "Advanced");
        stack.add_titled(&status_view, Some("status"), "Status");

        // Create status bar
        let status_bar = create_status_bar(&gui_state);
        main_box.append(&status_bar);

        window.set_child(Some(&main_box));
        
        // Initialize with config values
        populate_initial_values(&gui_state, &initial_config);
        
        window.present();
    });

    application.run();
    Ok(())
}

fn load_custom_css() {
    let css_provider = CssProvider::new();
    css_provider.load_from_data(include_str!("../assets/style.css"));
    
    StyleContext::add_provider_for_display(
        &gtk::gdk::Display::default().unwrap(),
        &css_provider,
        STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn create_title_widget() -> Box {
    let title_box = Box::new(Orientation::Horizontal, 8);
    
    let icon = Image::from_icon_name("applications-games");
    icon.set_pixel_size(24);
    title_box.append(&icon);
    
    let title_label = Label::new(Some("Hydra Co-op Launcher"));
    title_label.add_css_class("title-label");
    title_box.append(&title_label);
    
    title_box
}

fn create_menu_button() -> MenuButton {
    let menu_button = MenuButton::new();
    menu_button.set_icon_name("open-menu-symbolic");
    
    let popover = Popover::new();
    let menu_box = Box::new(Orientation::Vertical, 4);
    menu_box.set_margin_top(8);
    menu_box.set_margin_bottom(8);
    menu_box.set_margin_start(8);
    menu_box.set_margin_end(8);
    
    let about_button = Button::with_label("About");
    about_button.add_css_class("flat");
    about_button.connect_clicked(|_| {
        // Show about dialog
        show_about_dialog();
    });
    
    let help_button = Button::with_label("Help");
    help_button.add_css_class("flat");
    
    menu_box.append(&about_button);
    menu_box.append(&help_button);
    
    popover.set_child(Some(&menu_box));
    menu_button.set_popover(Some(&popover));
    
    menu_button
}

fn create_setup_view(gui_state: &Rc<RefCell<GuiState>>, initial_config: &Config) -> ScrolledWindow {
    let scrolled = ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    
    let main_grid = Grid::new();
    main_grid.set_row_spacing(16);
    main_grid.set_column_spacing(16);
    main_grid.set_margin_top(24);
    main_grid.set_margin_bottom(24);
    main_grid.set_margin_start(24);
    main_grid.set_margin_end(24);
    main_grid.add_css_class("setup-grid");

    let mut row = 0;

    // Game Selection Section
    let game_frame = create_section_frame("Game Selection", "Select the game executable to launch");
    let game_content = Box::new(Orientation::Vertical, 12);
    
    let file_selection_box = Box::new(Orientation::Horizontal, 12);
    let select_button = Button::with_label("Browse for Game");
    select_button.add_css_class("suggested-action");
    select_button.set_size_request(150, -1);
    
    let file_path_label = Label::new(Some("No game selected"));
    file_path_label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    file_path_label.set_halign(Align::Start);
    file_path_label.add_css_class("file-path-label");
    gui_state.borrow_mut().file_path_label = Some(file_path_label.clone());
    
    file_selection_box.append(&select_button);
    file_selection_box.append(&file_path_label);
    
    game_content.append(&file_selection_box);
    game_frame.set_child(Some(&game_content));
    main_grid.attach(&game_frame, 0, row, 2, 1);
    row += 1;

    // Players Configuration Section
    let players_frame = create_section_frame("Players Configuration", "Configure number of players and input devices");
    let players_content = Grid::new();
    players_content.set_row_spacing(12);
    players_content.set_column_spacing(16);
    
    let num_players_label = Label::new(Some("Number of Players:"));
    num_players_label.set_halign(Align::Start);
    num_players_label.add_css_class("setting-label");
    
    let num_players_combo = ComboBoxText::new();
    for i in 1..=8 {
        num_players_combo.append_text(&i.to_string());
    }
    num_players_combo.set_active(Some(1)); // Default to 2 players
    gui_state.borrow_mut().num_players_combo = Some(num_players_combo.clone());
    
    players_content.attach(&num_players_label, 0, 0, 1, 1);
    players_content.attach(&num_players_combo, 1, 0, 1, 1);
    
    // Profile name
    let profile_label = Label::new(Some("Profile Name:"));
    profile_label.set_halign(Align::Start);
    profile_label.add_css_class("setting-label");
    
    let profile_entry = Entry::new();
    profile_entry.set_placeholder_text(Some("Enter profile name (optional)"));
    gui_state.borrow_mut().profile_name_entry = Some(profile_entry.clone());
    
    players_content.attach(&profile_label, 0, 1, 1, 1);
    players_content.attach(&profile_entry, 1, 1, 1, 1);
    
    // Input devices container
    let input_label = Label::new(Some("Input Assignments:"));
    input_label.set_halign(Align::Start);
    input_label.set_valign(Align::Start);
    input_label.add_css_class("setting-label");
    
    let input_fields_container = Grid::new();
    input_fields_container.set_row_spacing(8);
    input_fields_container.set_column_spacing(12);
    gui_state.borrow_mut().input_fields_container = Some(input_fields_container.clone());
    
    players_content.attach(&input_label, 0, 2, 1, 1);
    players_content.attach(&input_fields_container, 1, 2, 1, 1);
    
    players_frame.set_child(Some(&players_content));
    main_grid.attach(&players_frame, 0, row, 2, 1);
    row += 1;

    // Layout Configuration Section
    let layout_frame = create_section_frame("Display Layout", "Choose how game windows are arranged");
    let layout_content = Box::new(Orientation::Horizontal, 16);
    
    let horizontal_radio = RadioButton::with_label(None, "Horizontal Split");
    let vertical_radio = RadioButton::with_label_from_widget(&horizontal_radio, "Vertical Split");
    let grid_radio = RadioButton::with_label_from_widget(&horizontal_radio, "2x2 Grid");
    
    horizontal_radio.add_css_class("layout-radio");
    vertical_radio.add_css_class("layout-radio");
    grid_radio.add_css_class("layout-radio");
    
    layout_content.append(&horizontal_radio);
    layout_content.append(&vertical_radio);
    layout_content.append(&grid_radio);
    
    gui_state.borrow_mut().layout_radios = vec![horizontal_radio.clone(), vertical_radio.clone(), grid_radio.clone()];
    
    layout_frame.set_child(Some(&layout_content));
    main_grid.attach(&layout_frame, 0, row, 2, 1);
    row += 1;

    // Advanced Options Section
    let advanced_frame = create_section_frame("Advanced Options", "Additional configuration options");
    let advanced_content = Box::new(Orientation::Vertical, 8);
    
    let proton_checkbox = CheckButton::with_label("Use Proton (for Windows games)");
    proton_checkbox.add_css_class("option-checkbox");
    gui_state.borrow_mut().use_proton_checkbox = Some(proton_checkbox.clone());
    
    advanced_content.append(&proton_checkbox);
    advanced_frame.set_child(Some(&advanced_content));
    main_grid.attach(&advanced_frame, 0, row, 2, 1);
    row += 1;

    // Action Buttons
    let button_box = Box::new(Orientation::Horizontal, 12);
    button_box.set_halign(Align::End);
    button_box.set_margin_top(24);
    
    let save_button = Button::with_label("Save Configuration");
    save_button.add_css_class("flat");
    
    let launch_button = Button::with_label("Launch Game");
    launch_button.add_css_class("suggested-action");
    launch_button.set_size_request(120, 40);
    gui_state.borrow_mut().launch_button = Some(launch_button.clone());
    
    button_box.append(&save_button);
    button_box.append(&launch_button);
    
    main_grid.attach(&button_box, 0, row, 2, 1);

    // Connect signals
    connect_setup_signals(gui_state, &select_button, &save_button, &launch_button, &num_players_combo);

    scrolled.set_child(Some(&main_grid));
    scrolled
}

fn create_advanced_view(gui_state: &Rc<RefCell<GuiState>>) -> ScrolledWindow {
    let scrolled = ScrolledWindow::new();
    
    let main_box = Box::new(Orientation::Vertical, 16);
    main_box.set_margin_top(24);
    main_box.set_margin_bottom(24);
    main_box.set_margin_start(24);
    main_box.set_margin_end(24);
    
    // Network Configuration
    let network_frame = create_section_frame("Network Configuration", "Configure network ports and settings");
    let network_grid = Grid::new();
    network_grid.set_row_spacing(8);
    network_grid.set_column_spacing(12);
    
    let port_label = Label::new(Some("Base Port:"));
    port_label.set_halign(Align::Start);
    let port_entry = Entry::new();
    port_entry.set_text("7777");
    port_entry.set_placeholder_text(Some("Starting port number"));
    
    network_grid.attach(&port_label, 0, 0, 1, 1);
    network_grid.attach(&port_entry, 1, 0, 1, 1);
    
    network_frame.set_child(Some(&network_grid));
    main_box.append(&network_frame);
    
    // Performance Settings
    let perf_frame = create_section_frame("Performance Settings", "Optimize for your system");
    let perf_grid = Grid::new();
    perf_grid.set_row_spacing(8);
    perf_grid.set_column_spacing(12);
    
    let cpu_label = Label::new(Some("CPU Priority:"));
    cpu_label.set_halign(Align::Start);
    let cpu_combo = ComboBoxText::new();
    cpu_combo.append_text("Normal");
    cpu_combo.append_text("High");
    cpu_combo.append_text("Real-time");
    cpu_combo.set_active(Some(0));
    
    perf_grid.attach(&cpu_label, 0, 0, 1, 1);
    perf_grid.attach(&cpu_combo, 1, 0, 1, 1);
    
    perf_frame.set_child(Some(&perf_grid));
    main_box.append(&perf_frame);
    
    scrolled.set_child(Some(&main_box));
    scrolled
}

fn create_status_view(gui_state: &Rc<RefCell<GuiState>>) -> ScrolledWindow {
    let scrolled = ScrolledWindow::new();
    
    let main_box = Box::new(Orientation::Vertical, 16);
    main_box.set_margin_top(24);
    main_box.set_margin_bottom(24);
    main_box.set_margin_start(24);
    main_box.set_margin_end(24);
    
    // Status Information
    let status_frame = create_section_frame("Launch Status", "Current operation status");
    let status_content = Box::new(Orientation::Vertical, 12);
    
    let status_label = Label::new(Some("Ready to launch"));
    status_label.set_halign(Align::Start);
    status_label.add_css_class("status-label");
    gui_state.borrow_mut().status_label = Some(status_label.clone());
    
    let progress_bar = ProgressBar::new();
    progress_bar.set_show_text(true);
    progress_bar.add_css_class("launch-progress");
    gui_state.borrow_mut().progress_bar = Some(progress_bar.clone());
    
    status_content.append(&status_label);
    status_content.append(&progress_bar);
    status_frame.set_child(Some(&status_content));
    main_box.append(&status_frame);
    
    // Log Output
    let log_frame = create_section_frame("Log Output", "Detailed launch information");
    let log_scrolled = ScrolledWindow::new();
    log_scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    log_scrolled.set_min_content_height(200);
    
    let log_view = TextView::new();
    log_view.set_editable(false);
    log_view.set_cursor_visible(false);
    log_view.add_css_class("log-view");
    
    let log_buffer = log_view.buffer();
    gui_state.borrow_mut().log_buffer = Some(log_buffer.clone());
    
    log_scrolled.set_child(Some(&log_view));
    log_frame.set_child(Some(&log_scrolled));
    main_box.append(&log_frame);
    
    scrolled.set_child(Some(&main_box));
    scrolled
}

fn create_section_frame(title: &str, subtitle: &str) -> Frame {
    let frame = Frame::new(None);
    frame.add_css_class("section-frame");
    
    let header_box = Box::new(Orientation::Vertical, 4);
    header_box.set_margin_top(8);
    header_box.set_margin_bottom(12);
    header_box.set_margin_start(12);
    header_box.set_margin_end(12);
    
    let title_label = Label::new(Some(title));
    title_label.set_halign(Align::Start);
    title_label.add_css_class("section-title");
    
    let subtitle_label = Label::new(Some(subtitle));
    subtitle_label.set_halign(Align::Start);
    subtitle_label.add_css_class("section-subtitle");
    
    header_box.append(&title_label);
    header_box.append(&subtitle_label);
    
    frame.set_label_widget(Some(&header_box));
    frame
}

fn create_status_bar(gui_state: &Rc<RefCell<GuiState>>) -> Box {
    let status_bar = Box::new(Orientation::Horizontal, 8);
    status_bar.set_margin_top(8);
    status_bar.set_margin_bottom(8);
    status_bar.set_margin_start(12);
    status_bar.set_margin_end(12);
    status_bar.add_css_class("status-bar");
    
    let status_icon = Image::from_icon_name("emblem-ok-symbolic");
    status_icon.set_pixel_size(16);
    
    let status_text = Label::new(Some("Ready"));
    status_text.add_css_class("status-text");
    
    status_bar.append(&status_icon);
    status_bar.append(&status_text);
    
    // Add spacer
    let spacer = Label::new(None);
    spacer.set_hexpand(true);
    status_bar.append(&spacer);
    
    // Add version info
    let version_label = Label::new(Some(&format!("v{}", env!("CARGO_PKG_VERSION"))));
    version_label.add_css_class("version-label");
    status_bar.append(&version_label);
    
    status_bar
}

fn connect_setup_signals(
    gui_state: &Rc<RefCell<GuiState>>,
    select_button: &Button,
    save_button: &Button,
    launch_button: &Button,
    num_players_combo: &ComboBoxText,
) {
    // File selection
    let gui_state_file = Rc::clone(gui_state);
    select_button.connect_clicked(move |_| {
        let state = gui_state_file.borrow();
        let window = state.main_window.as_ref().unwrap();
        
        let dialog = FileChooserDialog::builder()
            .title("Select Game Executable")
            .action(gtk::FileChooserAction::Open)
            .modal(true)
            .transient_for(window)
            .build();

        dialog.add_button("Cancel", gtk::ResponseType::Cancel);
        dialog.add_button("Open", gtk::ResponseType::Accept);

        let gui_state_dialog = Rc::clone(&gui_state_file);
        dialog.connect_response(move |dialog, response| {
            if response == gtk::ResponseType::Accept {
                if let Some(file) = dialog.file() {
                    if let Some(path) = file.path() {
                        let state = gui_state_dialog.borrow();
                        if let Some(label) = &state.file_path_label {
                            label.set_text(&path.to_string_lossy());
                        }
                    }
                }
            }
            dialog.close();
        });
        
        dialog.show();
    });

    // Number of players change
    let gui_state_players = Rc::clone(gui_state);
    num_players_combo.connect_changed(move |combo| {
        if let Some(text) = combo.active_text() {
            if let Ok(num_players) = text.parse::<usize>() {
                update_input_fields(&gui_state_players, num_players);
            }
        }
    });

    // Save configuration
    let gui_state_save = Rc::clone(gui_state);
    save_button.connect_clicked(move |_| {
        save_configuration(&gui_state_save);
    });

    // Launch game
    let gui_state_launch = Rc::clone(gui_state);
    launch_button.connect_clicked(move |_| {
        launch_game(&gui_state_launch);
    });
}

fn update_input_fields(gui_state: &Rc<RefCell<GuiState>>, num_players: usize) {
    let mut state = gui_state.borrow_mut();
    let container = state.input_fields_container.as_ref().unwrap();
    
    // Clear existing widgets
    let mut child = container.first_child();
    while let Some(widget) = child {
        let next = widget.next_sibling();
        container.remove(&widget);
        child = next;
    }
    
    state.input_combos.clear();
    
    // Create new input assignments
    for i in 0..num_players {
        let player_label = Label::new(Some(&format!("Player {}:", i + 1)));
        player_label.set_halign(Align::Start);
        player_label.add_css_class("player-label");
        
        let input_combo = ComboBoxText::new();
        input_combo.append_text("Auto-detect");
        
        for device in &state.available_input_devices {
            input_combo.append(&serde_json::to_string(device).unwrap_or_default(), &device.name);
        }
        
        input_combo.set_active(Some(0));
        input_combo.add_css_class("input-combo");
        
        container.attach(&player_label, 0, i as i32, 1, 1);
        container.attach(&input_combo, 1, i as i32, 1, 1);
        
        state.input_combos.push(input_combo);
    }
}

fn save_configuration(gui_state: &Rc<RefCell<GuiState>>) {
    let state = gui_state.borrow();
    info!("Saving configuration...");
    
    // Implementation for saving configuration
    if let Some(window) = &state.main_window {
        show_info_dialog(window, "Configuration Saved", "Your settings have been saved successfully.");
    }
}

fn launch_game(gui_state: &Rc<RefCell<GuiState>>) {
    let mut state = gui_state.borrow_mut();
    info!("Launching game...");
    
    // Switch to status view
    if let Some(stack) = &state.stack {
        stack.set_visible_child_name("status");
    }
    
    // Update status
    if let Some(status_label) = &state.status_label {
        status_label.set_text("Launching game instances...");
    }
    
    if let Some(progress_bar) = &state.progress_bar {
        progress_bar.set_fraction(0.0);
        progress_bar.set_text(Some("Initializing..."));
    }
    
    // Disable launch button
    if let Some(launch_button) = &state.launch_button {
        launch_button.set_sensitive(false);
    }
    
    // Add log message
    if let Some(log_buffer) = &state.log_buffer {
        let mut end_iter = log_buffer.end_iter();
        log_buffer.insert(&mut end_iter, "Starting game launch process...\n");
    }
    
    // TODO: Implement actual game launching logic
    // This would call run_core_logic in a separate thread
}

fn populate_initial_values(gui_state: &Rc<RefCell<GuiState>>, config: &Config) {
    let state = gui_state.borrow();
    
    // Set game path
    if let Some(game_path) = config.game_paths.first() {
        if let Some(label) = &state.file_path_label {
            label.set_text(&game_path.to_string_lossy());
        }
    }
    
    // Set number of players
    if let Some(combo) = &state.num_players_combo {
        let player_count = config.input_mappings.len().max(1);
        combo.set_active(Some((player_count - 1) as u32));
    }
    
    // Set layout
    match config.window_layout.as_str() {
        "horizontal" => state.layout_radios[0].set_active(true),
        "vertical" => state.layout_radios[1].set_active(true),
        "grid2x2" => state.layout_radios[2].set_active(true),
        _ => state.layout_radios[0].set_active(true),
    }
    
    // Set Proton checkbox
    if let Some(checkbox) = &state.use_proton_checkbox {
        checkbox.set_active(config.use_proton);
    }
}

fn show_about_dialog() {
    let dialog = MessageDialog::new(
        None::<&ApplicationWindow>,
        DialogFlags::MODAL,
        MessageType::Info,
        ButtonsType::Close,
        &format!("Hydra Co-op Launcher v{}\n\nA universal tool for local split-screen co-operative gameplay.", env!("CARGO_PKG_VERSION")),
    );
    dialog.set_title(Some("About Hydra Co-op Launcher"));
    dialog.connect_response(|dialog, _| dialog.close());
    dialog.show();
}

fn show_error_dialog(parent_window: &ApplicationWindow, title: &str, message: &str) {
    let dialog = MessageDialog::new(
        Some(parent_window),
        DialogFlags::MODAL,
        MessageType::Error,
        ButtonsType::Close,
        message,
    );
    dialog.set_title(Some(title));
    dialog.connect_response(|dialog, _| dialog.close());
    dialog.show();
}

fn show_warning_dialog(parent_window: &ApplicationWindow, title: &str, message: &str) {
    let dialog = MessageDialog::new(
        Some(parent_window),
        DialogFlags::MODAL,
        MessageType::Warning,
        ButtonsType::Close,
        message,
    );
    dialog.set_title(Some(title));
    dialog.connect_response(|dialog, _| dialog.close());
    dialog.show();
}

fn show_info_dialog(parent_window: &ApplicationWindow, title: &str, message: &str) {
    let dialog = MessageDialog::new(
        Some(parent_window),
        DialogFlags::MODAL,
        MessageType::Info,
        ButtonsType::Close,
        message,
    );
    dialog.set_title(Some(title));
    dialog.connect_response(|dialog, _| dialog.close());
    dialog.show();
}