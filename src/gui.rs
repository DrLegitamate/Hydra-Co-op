use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Grid, ComboBoxText, Entry, Button, Label, RadioButton, FileChooserDialog};
use crate::input_mux::{InputMux, DeviceIdentifier}; // Import DeviceIdentifier
use log::{info, error, warn};
use std::rc::Rc; // Use Rc for shared ownership in a single-threaded context (GUI)
use std::cell::RefCell; // Use RefCell for mutable interior
use std::path::PathBuf; // Import PathBuf
use crate::config::Config; // Import Config
use crate::window_manager::Layout; // Import Layout enum (or a GUI representation)
use std::collections::HashMap; // Import HashMap

// Define a struct to hold GUI state and data accessible by signal handlers
#[derive(Clone, Default)] // Derive Clone and Default for easier initialization
struct GuiState {
    available_input_devices: Vec<DeviceIdentifier>,
    // Add other fields to store widget references or collected data temporarily
    file_path_label: Option<Label>, // Store reference to the label displaying the file path
    num_players_combo: Option<ComboBoxText>,
    input_combos: Vec<ComboBoxText>, // Store input combo boxes dynamically
    layout_radios: Vec<RadioButton>,
    profile_name_entry: Option<Entry>,
    // Add references to buttons if needed
}


/// Builds and runs the GTK application GUI.
///
/// # Arguments
///
/// * `initial_config` - The configuration loaded at application startup (e.g., from config file).
/// * `available_devices` - List of input devices enumerated at startup.
/// * `on_launch` - A callback function to trigger the core launch logic.
// pub fn run_gui(initial_config: Config, available_devices: Vec<DeviceIdentifier>, on_launch: impl Fn(Config, Vec<DeviceIdentifier>, Layout, bool) + 'static) -> Result<(), Box<dyn std::error::Error>> { // Adjust function signature and parameters
pub fn run_gui() -> Result<(), Box<dyn std::error::Error>> { // Simplified signature for now

    // TODO: Initialize logging before calling run_gui in main.rs

    let application = Application::new(
        Some("com.example.split_screen_launcher.gui"),
        Default::default(),
    );

    // Use Rc<RefCell<>> for shared mutable state in the single-threaded GUI context
    let gui_state = Rc::new(RefCell::new(GuiState::default()));


    application.connect_activate(move |app| {
        let window = ApplicationWindow::new(app);
        window.set_title("Hydra Co-op Launcher");
        window.set_default_size(800, 600); // Adjust default size


        let grid_container = Grid::new();
        grid_container.set_row_spacing(10);
        grid_container.set_column_spacing(10);
        grid_container.set_margin_top(10);
        grid_container.set_margin_bottom(10);
        grid_container.set_margin_start(10);
        grid_container.set_margin_end(10);
        window.set_child(Some(&grid_container)); // Use set_child for Gtk 4+

        // Populate gui_state with widget references (optional, can capture in closures)
        let mut state = gui_state.borrow_mut();


        // --- Number of Players ---
        let num_players_label = gtk::Label::new(Some("Number of Players:"));
        grid_container.attach(&num_players_label, 0, 0, 1, 1);

        let num_players_combo = gtk::ComboBoxText::new();
        for i in 2..=4 { // Support 2 to 4 players example
            num_players_combo.append_text(&i.to_string());
        }
        grid_container.attach(&num_players_combo, 1, 0, 1, 1);
        num_players_combo.set_active_id(Some("2")); // Default to 2 players


        // --- Profile Name ---
        let profile_name_label = gtk::Label::new(Some("Profile Name:"));
        grid_container.attach(&profile_name_label, 0, 1, 1, 1);

        let profile_name_entry = gtk::Entry::new();
        profile_name_entry.set_placeholder_text(Some("Enter profile name"));
        grid_container.attach(&profile_name_entry, 1, 1, 1, 1);


        // --- Game Executable ---
        let select_button = gtk::Button::with_label("Select Game Executable");
        grid_container.attach(&select_button, 0, 2, 1, 1);

        let file_path_label = gtk::Label::new(None);
        file_path_label.set_ellipsize(pango::EllipsizeMode::Start); // Ellipsize long paths
        grid_container.attach(&file_path_label, 1, 2, 1, 1);


        // --- Layout Selection ---
        let layout_label = gtk::Label::new(Some("Split-Screen Layout:"));
        grid_container.attach(&layout_label, 0, 3, 1, 1);

        let layout_box = gtk::Box::new(gtk::Orientation::Horizontal, 5); // Use a horizontal box for radio buttons
        grid_container.attach(&layout_box, 1, 3, 1, 1);

        let horizontal_radio = gtk::RadioButton::with_label(None, "Horizontal");
        let vertical_radio = gtk::RadioButton::with_label_from_widget(&horizontal_radio, "Vertical");
        let custom_radio = gtk::RadioButton::with_label_from_widget(&horizontal_radio, "Custom");

        layout_box.append(&horizontal_radio); // Use append for Gtk 4+
        layout_box.append(&vertical_radio);
        layout_box.append(&custom_radio);

        horizontal_radio.set_active(true); // Default layout


        // --- Input Device Assignment (Dynamic) ---
        // This part needs to be dynamic based on num_players_combo selection
        // We'll add placeholders here and illustrate how to make it dynamic.
        let input_assignment_label = gtk::Label::new(Some("Input Assignments:"));
        grid_container.attach(&input_assignment_label, 0, 4, 1, 2); // Span multiple rows


        // Placeholder: Container for dynamically added input fields
        let input_fields_container = Grid::new(); // Use a Grid for input fields
        input_fields_container.set_row_spacing(5);
        input_fields_container.set_column_spacing(5);
        grid_container.attach(&input_fields_container, 1, 4, 1, 4); // Position and span

        // TODO: Implement logic to clear and repopulate input_fields_container
        // when the "Number of Players" combo box selection changes.

        // Function to populate input device combo box (needs access to available devices)
        let populate_input_combo = |combo: &gtk::ComboBoxText, available_devices: &[DeviceIdentifier]| {
             combo.remove_all(); // Clear existing items
             combo.append_text("Auto-detect"); // Option for auto-detection
             for device_id in available_devices {
                 combo.append_text(&device_id.name); // Add device names
             }
             combo.set_active(0); // Default to "Auto-detect"
        };


        // Initial population for a default number of players (e.g., 2)
        let initial_num_players = 2; // Match the default in num_players_combo
        for i in 0..initial_num_players {
            let player_label = gtk::Label::new(Some(&format!("Player {}:", i + 1)));
            input_fields_container.attach(&player_label, 0, i as i32, 1, 1);

            let input_combo = gtk::ComboBoxText::new();
            // Populate this combo box with available input devices
             // This requires access to the enumerated devices, which should be done ONCE.
             // Example call (assuming available_devices is accessible):
             // populate_input_combo(&input_combo, &state.available_input_devices);

            input_fields_container.attach(&input_combo, 1, i as i32, 1, 1);
            // Store reference to the combo box for data retrieval later
             state.input_combos.push(input_combo);
        }


        // --- Control Buttons ---
        let buttons_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        grid_container.attach(&buttons_box, 0, 8, 2, 1); // Span across columns
        buttons_box.set_halign(gtk::Align::End); // Align buttons to the end

        let save_button = gtk::Button::with_label("Launch Game"); // Changed label to "Launch Game"
        let cancel_button = gtk::Button::with_label("Cancel");

        buttons_box.append(&cancel_button);
        buttons_box.append(&save_button);

        // --- Event Handling ---

        // Select Game Executable Button
        select_button.connect_clicked(move |btn| {
            let window = btn.ancestor().unwrap().downcast::<ApplicationWindow>().unwrap();
            let dialog = gtk::FileChooserDialog::builder() // Use builder for Gtk 4+
                .title("Select Game Executable")
                .action(gtk::FileChooserAction::Open)
                .modal(true) // Make the dialog modal
                .transient_for(&window) // Link to the main window
                .build();

            dialog.add_button("Open", gtk::ResponseType::Accept);
            dialog.add_button("Cancel", gtk::ResponseType::Cancel);

            dialog.connect_response(move |dialog, response| {
                if response == gtk::ResponseType::Accept {
                    if let Some(file) = dialog.file() { // Use file() for Gtk 4+
                        if let Some(path) = file.path() {
                            file_path_label.set_text(&path.to_string_lossy());
                        }
                    }
                }
                dialog.close();
            });
            dialog.show();
        });


        // Launch Game Button (formerly Save Settings)
        let gui_state_clone = Rc::clone(&gui_state); // Clone Rc for the closure
        save_button.connect_clicked(move |_| {
            let state = gui_state_clone.borrow(); // Borrow the state mutably

            // Collect data from widgets
            let file_path = PathBuf::from(state.file_path_label.as_ref().unwrap().get_text().to_string());
            let player_count_str = state.num_players_combo.as_ref().unwrap().get_active_text().unwrap_or_else(|| "2".to_string());
            let player_count = player_count_str.parse::<usize>().unwrap_or(2); // Parse or default

            let mut input_assignments: Vec<String> = Vec::new();
            // Iterate through dynamically created input combo boxes
             for combo in &state.input_combos {
                 input_assignments.push(combo.get_active_text().unwrap_or_else(|| "".to_string()));
             }
             // Trim input assignments to match player count if needed
             input_assignments.truncate(player_count);


            let layout_option = if state.horizontal_radio.as_ref().unwrap().get_active() {
                "Horizontal"
            } else if state.vertical_radio.as_ref().unwrap().get_active() {
                "Vertical"
            } else {
                "Custom"
            };
            let layout = Layout::from(layout_option); // Convert to your Layout enum

            let profile_name = state.profile_name_entry.as_ref().unwrap().get_text().to_string();

            info!("--- GUI Settings Collected ---");
            info!("File Path: {}", file_path.display());
            info!("Player Count: {}", player_count);
            info!("Input Assignments: {:?}", input_assignments); // These are device NAMES or "Auto-detect"
            info!("Layout Option: {:?}", layout);
            info!("Profile Name: {}", profile_name);
            info!("------------------------------");

            // TODO: Validate collected data (e.g., file path exists, player count > 0)
            // TODO: Trigger the core application launch logic (call functions from main.rs or an application controller)
            // This would involve passing the collected settings (file_path, player_count, input_assignments, layout, profile_name)
            // to the functions responsible for launching instances, setting up network, etc.
            // The input_assignments here are strings (names/auto-detect). You'll need to map these back
            // to DeviceIdentifier when setting up InputMux.
        });


        // Cancel Button
        let window_clone = window.clone(); // Clone window for the closure
        cancel_button.connect_clicked(move |_| {
            info!("Cancel button clicked. Closing window.");
            window_clone.close();
        });


        // TODO: Implement dynamic input device field creation based on player count selection
        // This would involve connecting a signal to the "Number of Players" combo box
        // and updating the `input_fields_container`.

        // Initial population of input combo boxes with available devices
         // This needs to be done once after enumerating devices.
         // Where is input enumeration happening now? It was in build_ui.
         // It should happen *before* run_gui is called, and the list of devices
         // should be passed to run_gui or accessible via the shared state.

         // Example: Enumerate devices here or before calling run_gui
         let mut input_mux_enumerator = InputMux::new();
         let available_devices = match input_mux_enumerator.enumerate_devices() {
             Ok(_) => {
                  info!("Input devices enumerated for GUI.");
                  input_mux_enumerator.get_available_devices()
             }
             Err(e) => {
                  error!("Failed to enumerate input devices for GUI: {}", e);
                  // Display an error to the user in the GUI
                  // Returning an empty list allows the GUI to still start.
                  Vec::new()
             }
         };
          // Store available devices in the shared state
          gui_state.borrow_mut().available_input_devices = available_devices.clone(); // Clone for storage

         // Now populate the initially created input combos
         let mut state = gui_state.borrow_mut();
         for combo in &state.input_combos {
             populate_input_combo(combo, &available_devices);
         }


        window.present(); // Use present() for Gtk 4+
    });

    application.run();

    Ok(()) // Return Ok on successful application run
}

// Note: This file should likely be in src/gui.rs or similar.
// The main function in src/main.rs would then decide whether to run the CLI
// or the GUI (by calling run_gui()).

// Example of how main.rs might call run_gui:
/*
fn main() {
    // ... logging init ...
    // ... parse args to check if GUI mode is requested ...

    let use_gui = true; // Determine this from args or config

    if use_gui {
        info!("Starting GUI mode.");
         if let Err(e) = gui::run_gui() { // Call the public function from gui.rs
             error!("GUI application failed: {}", e);
             std::process::exit(1);
         }
    } else {
        info!("Starting CLI mode.");
        // ... execute existing CLI logic ...
    }
}
*/
