use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Grid, ComboBoxText, Entry, Button, Label, RadioButton, FileChooserDialog, Align, Orientation}; // Import Align, Orientation
use crate::input_mux::{InputMux, DeviceIdentifier}; // Import DeviceIdentifier
use log::{info, error, warn, debug}; // Import debug
use std::rc::Rc; // Use Rc for shared ownership in a single-threaded context (GUI)
use std::cell::RefCell; // Use RefCell for mutable interior
use std::path::PathBuf; // Import PathBuf
use crate::config::Config; // Import Config
use crate::window_manager::Layout; // Import Layout enum (or a GUI representation)
use std::collections::HashMap; // Import HashMap
use crate::run_core_logic; // Import the core logic function from main.rs

// Define a struct to hold GUI state and data accessible by signal handlers
// #[derive(Clone, Default)] // Default derive is okay, but Clone might not be needed with Rc
#[derive(Default)]
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
/// * `available_devices` - List of input devices enumerated at startup (passed from main.rs).
/// * `initial_config` - The configuration loaded at application startup (passed from main.rs).
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - Returns Ok on successful application run.
pub fn run_gui(available_devices: Vec<DeviceIdentifier>, initial_config: Config) -> Result<(), Box<dyn std::error::Error>> { // Added parameters

    // TODO: Logging is assumed to be initialized in main.rs before calling run_gui


    let application = Application::new(
        Some("com.example.split_screen_launcher.gui"),
        Default::default(),
    );

    // Use Rc<RefCell<>> for shared mutable state in the single-threaded GUI context
    let gui_state = Rc::new(RefCell::new(GuiState::default()));
     // Store available devices in the shared state upon GUI startup
     gui_state.borrow_mut().available_input_devices = available_devices.clone(); // Clone for storage


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


        // --- Number of Players ---
        let num_players_label = gtk::Label::new(Some("Number of Players:"));
        grid_container.attach(&num_players_label, 0, 0, 1, 1);

        let num_players_combo = gtk::ComboBoxText::new();
        for i in 2..=4 { // Support 2 to 4 players example
            num_players_combo.append_text(&i.to_string());
        }
        grid_container.attach(&num_players_combo, 1, 0, 1, 1);
        num_players_combo.set_active_id(Some("2")); // Default to 2 players
         gui_state.borrow_mut().num_players_combo = Some(num_players_combo.clone()); // Store reference


        // --- Profile Name ---
        let profile_name_label = gtk::Label::new(Some("Profile Name:"));
        grid_container.attach(&profile_name_label, 0, 1, 1, 1);

        let profile_name_entry = gtk::Entry::new();
        profile_name_entry.set_placeholder_text(Some("Enter profile name"));
        grid_container.attach(&profile_name_entry, 1, 1, 1, 1);
         gui_state.borrow_mut().profile_name_entry = Some(profile_name_entry.clone()); // Store reference


        // --- Game Executable ---
        let select_button = gtk::Button::with_label("Select Game Executable");
        grid_container.attach(&select_button, 0, 2, 1, 1);

        let file_path_label = gtk::Label::new(None);
        file_path_label.set_ellipsize(pango::EllipsizeMode::Start); // Ellipsize long paths
        grid_container.attach(&file_path_label, 1, 2, 1, 1);
         gui_state.borrow_mut().file_path_label = Some(file_path_label.clone()); // Store reference


        // --- Layout Selection ---
        let layout_label = gtk::Label::new(Some("Split-Screen Layout:"));
        grid_container.attach(&layout_label, 0, 3, 1, 1);

        let layout_box = gtk::Box::new(Orientation::Horizontal, 5); // Use a horizontal box for radio buttons
        grid_container.attach(&layout_box, 1, 3, 1, 1);

        let horizontal_radio = gtk::RadioButton::with_label(None, "Horizontal");
        let vertical_radio = gtk::RadioButton::with_label_from_widget(&horizontal_radio, "Vertical");
        let custom_radio = gtk::RadioButton::with_label_from_widget(&horizontal_radio, "Custom");

        layout_box.append(&horizontal_radio); // Use append for Gtk 4+
        layout_box.append(&vertical_radio);
        layout_box.append(&custom_radio);

        horizontal_radio.set_active(true); // Default layout
         gui_state.borrow_mut().layout_radios = vec![horizontal_radio, vertical_radio, custom_radio]; // Store references


        // --- Input Device Assignment (Dynamic Placeholder) ---
        let input_assignment_label = gtk::Label::new(Some("Input Assignments:"));
        grid_container.attach(&input_assignment_label, 0, 4, 1, 4); // Span multiple rows

        let input_fields_container = Grid::new(); // Use a Grid for input fields
        input_fields_container.set_row_spacing(5);
        input_fields_container.set_column_spacing(5);
        grid_container.attach(&input_fields_container, 1, 4, 1, 4); // Position and span

        // Store a reference to the input fields container to modify it later
        let input_fields_container_rc = Rc::new(input_fields_container);
         grid_container.attach(&*input_fields_container_rc, 1, 4, 1, 4); // Re-attach the Rc wrapped container


        // Function to populate input device combo box (needs access to available devices)
        let populate_input_combo = |combo: &gtk::ComboBoxText, available_devices: &[DeviceIdentifier]| {
             combo.remove_all(); // Clear existing items
             combo.append_text("Auto-detect"); // Option for auto-detection
             for device_id in available_devices {
                 combo.append_text(&device_id.name); // Add device names
             }
             combo.set_active(0); // Default to "Auto-detect"
        };

        // Function to update the dynamic input fields based on player count
        let update_input_fields = move |num_players: usize, container: &Grid, available_devices: &[DeviceIdentifier], gui_state: Rc<RefCell<GuiState>>| {
            info!("Updating input fields for {} players.", num_players);
            // Remove existing input fields
            for child in container.children() {
                container.remove(&child); // Use remove for Gtk 4+
            }
             gui_state.borrow_mut().input_combos.clear(); // Clear stored combo box references

            // Add new input fields based on the number of players
            for i in 0..num_players {
                let player_label = gtk::Label::new(Some(&format!("Player {}:", i + 1)));
                container.attach(&player_label, 0, i as i32, 1, 1);

                let input_combo = gtk::ComboBoxText::new();
                populate_input_combo(&input_combo, available_devices);
                container.attach(&input_combo, 1, i as i32, 1, 1);

                gui_state.borrow_mut().input_combos.push(input_combo); // Store reference
            }
             container.show_all(); // Show the new widgets
             info!("Input fields updated.");
        };

        // Connect signal to "Number of Players" combo box to update input fields dynamically
        let gui_state_clone_fields = Rc::clone(&gui_state);
        let available_devices_clone_fields = available_devices.clone();
        let input_fields_container_clone_fields = Rc::clone(&input_fields_container_rc);

        num_players_combo.connect_changed(move |combo| {
             if let Some(player_count_str) = combo.get_active_text() {
                 if let Ok(num_players) = player_count_str.parse::<usize>() {
                      update_input_fields(
                          num_players,
                          &*input_fields_container_clone_fields, // Deref Rc to pass Grid reference
                          &available_devices_clone_fields,
                          Rc::clone(&gui_state_clone_fields), // Pass cloned Rc
                       );
                 }
             }
        });

        // Initial update of input fields based on the default player count
        let initial_player_count_str = num_players_combo.get_active_text().unwrap_or_else(|| "2".to_string());
        let initial_player_count = initial_player_count_str.parse::<usize>().unwrap_or(2);
         // Pass cloned Rc explicitly
         update_input_fields(initial_player_count, &*input_fields_container_rc, &available_devices, Rc::clone(&gui_state));



        // --- Control Buttons ---
        let buttons_box = gtk::Box::new(Orientation::Horizontal, 10);
        grid_container.attach(&buttons_box, 0, 8, 2, 1); // Span across columns
        buttons_box.set_halign(Align::End); // Align buttons to the end

        let launch_button = gtk::Button::with_label("Launch Game");
        let cancel_button = gtk::Button::with_label("Cancel");

        buttons_box.append(&cancel_button);
        buttons_box.append(&launch_button);


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

            let file_path_label_clone = file_path_label.clone(); // Clone label for the closure
            dialog.add_button("Open", gtk::ResponseType::Accept);
            dialog.add_button("Cancel", gtk::ResponseType::Cancel);

            dialog.connect_response(move |dialog, response| {
                if response == gtk::ResponseType::Accept {
                    if let Some(file) = dialog.file() { // Use file() for Gtk 4+
                        if let Some(path) = file.path() {
                            file_path_label_clone.set_text(&path.to_string_lossy());
                        }
                    }
                }
                dialog.close();
            });
            dialog.show();
        });


        // Launch Game Button
        let gui_state_clone_launch = Rc::clone(&gui_state); // Clone Rc for the launch closure
         let initial_config_clone = initial_config.clone(); // Clone initial config for the closure

        launch_button.connect_clicked(move |_| {
            let state = gui_state_clone_launch.borrow(); // Borrow the state mutably

            // Collect data from widgets
            let file_path_str = state.file_path_label.as_ref().unwrap().get_text().to_string();
            if file_path_str.is_empty() {
                 warn!("Game executable path not selected. Cannot launch.");
                 // TODO: Show an error dialog to the user
                 return; // Stop here if no path is selected
            }
            let file_path = PathBuf::from(file_path_str);


            let player_count_str = state.num_players_combo.as_ref().unwrap().get_active_text().unwrap_or_else(|| "2".to_string());
            let player_count = player_count_str.parse::<usize>().unwrap_or(2);


            let mut input_assignments: Vec<String> = Vec::new();
             for combo in &state.input_combos {
                 input_assignments.push(combo.get_active_text().unwrap_or_else(|| "".to_string()));
             }
             // Trim input assignments to match player count
             input_assignments.truncate(player_count);


            let layout_option = if state.layout_radios[0].get_active() { // Horizontal
                "horizontal"
            } else if state.layout_radios[1].get_active() { // Vertical
                "vertical"
            } else { // Custom
                "custom"
            };
            let layout = Layout::from(layout_option); // Convert to your Layout enum


            let profile_name = state.profile_name_entry.as_ref().unwrap().get_text().to_string();

            // TODO: Implement logic to get the use_proton flag from the GUI (e.g., a checkbox)
            let use_proton = false; // Placeholder - needs a GUI control


            info!("--- GUI Settings Collected for Launch ---");
            info!("File Path: {}", file_path.display());
            info!("Player Count: {}", player_count);
            info!("Input Assignments (Names/Auto): {:?}", input_assignments);
            info!("Layout Option: {:?}", layout);
            info!("Profile Name: {}", profile_name);
            info!("Use Proton (Placeholder): {}", use_proton);
            info!("-----------------------------------------");

            // TODO: Validate collected data more thoroughly

            // Prepare input devices for run_core_logic
            // The input_assignments collected are device NAMES or "Auto-detect".
            // run_core_logic expects &[&str] which are device NAMES.
            // We need to pass the selected device names to run_core_logic.
             let input_device_names_for_core: Vec<&str> = input_assignments.iter().map(|s| s.as_str()).collect();


            // Trigger the core application launch logic in a separate thread
            // to keep the GUI responsive.
             let file_path_clone = file_path.clone(); // Clone PathBuf for the thread
             let initial_config_clone_for_thread = initial_config_clone.clone(); // Clone config for the thread
             let input_device_names_clone_for_thread: Vec<String> = input_assignments.clone(); // Clone Strings for the thread

             thread::spawn(move || {
                 info!("Launching core logic from GUI thread.");
                 // Convert Vec<String> back to Vec<&str> for the core logic function
                 let input_device_names_slice: Vec<&str> = input_device_names_clone_for_thread.iter().map(|s| s.as_str()).collect();

                 match run_core_logic(
                    &file_path_clone,
                    player_count,
                    &input_device_names_slice, // Pass the collected device names slice
                    layout, // Pass the collected layout
                    use_proton, // Pass the use_proton flag (placeholder)
                    &initial_config_clone_for_thread, // Pass the loaded config
                    // Pass other necessary data
                 ) {
                    Ok(_) => info!("Core application logic completed successfully."),
                    Err(e) => {
                        error!("Core application logic failed: {}", e);
                        // TODO: Display this error to the user in the GUI (e.g., a dialog box)
                    }
                 }
                 info!("Core logic thread finished.");
             });

        });


        // Cancel Button
        let window_clone = window.clone(); // Clone window for the closure
        cancel_button.connect_clicked(move |_| {
            info!("Cancel button clicked. Closing window.");
            // TODO: Implement graceful shutdown of background threads before closing
            window_clone.close();
        });


        // TODO: Implement loading initial_config into the GUI widgets


        window.present(); // Use present() for Gtk 4+
    });

    application.run();

    Ok(()) // Return Ok on successful application run
}

// Helper function to populate input device combo box (needs access to available devices)
// Moved inside run_gui as a local function or closure, or could be a method
// of GuiState if GuiState holds the available devices.

/*
fn populate_input_combo(combo: &gtk::ComboBoxText, available_devices: &[DeviceIdentifier]) {
    combo.remove_all(); // Clear existing items
    combo.append_text("Auto-detect"); // Option for auto-detection
    for device_id in available_devices {
        combo.append_text(&device_id.name); // Add device names
    }
    combo.set_active(0); // Default to "Auto-detect"
}
*/

// Helper function to update the dynamic input fields based on player count
// Moved inside run_gui as a local function or closure, or could be a method
// of GuiState if GuiState holds the widget references.

/*
fn update_input_fields(num_players: usize, container: &Grid, available_devices: &[DeviceIdentifier], gui_state: Rc<RefCell<GuiState>>) {
    info!("Updating input fields for {} players.", num_players);
    // Remove existing input fields
    for child in container.children() {
        container.remove(&child); // Use remove for Gtk 4+
    }
     gui_state.borrow_mut().input_combos.clear(); // Clear stored combo box references

    // Add new input fields based on the number of players
    for i in 0..num_players {
        let player_label = gtk::Label::new(Some(&format!("Player {}:", i + 1)));
        container.attach(&player_label, 0, i as i32, 1, 1);

        let input_combo = gtk::ComboBoxText::new();
        populate_input_combo(&input_combo, available_devices); // Use the populate helper
        container.attach(&input_combo, 1, i as i32, 1, 1);

        gui_state.borrow_mut().input_combos.push(input_combo); // Store reference
    }
     container.show_all(); // Show the new widgets
     info!("Input fields updated.");
}
*/


// Note: This file should likely be in src/gui.rs or similar.
// The main function in src/main.rs would then decide whether to run the CLI
// or the GUI (by calling run_gui()).
