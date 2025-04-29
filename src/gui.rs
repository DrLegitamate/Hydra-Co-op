use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Grid, ComboBoxText, Entry, Button, Label, RadioButton, FileChooserDialog, Align, Orientation};
use crate::input_mux::{InputMux, DeviceIdentifier};
use log::{info, error, warn, debug};
use std::rc::Rc;
use std::cell::RefCell;
use std::path::PathBuf;
use crate::config::Config;
use crate::window_manager::Layout;
use std::collections::HashMap;
use crate::run_core_logic;
use std::thread; // Import thread for spawning core logic


// Define a struct to hold GUI state and data accessible by signal handlers
#[derive(Default)]
struct GuiState {
    available_input_devices: Vec<DeviceIdentifier>,
    // Add other fields to store widget references or collected data temporarily
    file_path_label: Option<Label>,
    num_players_combo: Option<ComboBoxText>,
    input_combos: Vec<ComboBoxText>, // Store references to dynamically created input combo boxes
    layout_radios: Vec<RadioButton>,
    profile_name_entry: Option<Entry>,
    // Store reference to the container that holds the dynamic input fields
    input_fields_container: Option<Grid>,
    // Store reference to the main window for dialogs
    main_window: Option<ApplicationWindow>,
    // Store initial config for persistence and defaults
    initial_config: Config,
    // Store the list of available input devices
    // available_input_devices: already present
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
pub fn run_gui(available_devices: Vec<DeviceIdentifier>, initial_config: Config) -> Result<(), Box<dyn std::error::Error>> {

    let application = Application::new(
        Some("com.example.split_screen_launcher.gui"),
        Default::default(),
    );

    let gui_state = Rc::new(RefCell::new(GuiState::default()));
    gui_state.borrow_mut().available_input_devices = available_devices.clone();
    gui_state.borrow_mut().initial_config = initial_config.clone(); // Store initial config


    application.connect_activate(move |app| {
        let window = ApplicationWindow::new(app);
        window.set_title("Hydra Co-op Launcher");
        window.set_default_size(800, 600);
         gui_state.borrow_mut().main_window = Some(window.clone()); // Store window reference


        let grid_container = Grid::new();
        grid_container.set_row_spacing(10);
        grid_container.set_column_spacing(10);
        grid_container.set_margin_top(10);
        grid_container.set_margin_bottom(10);
        grid_container.set_margin_start(10);
        grid_container.set_margin_end(10);
        window.set_child(Some(&grid_container));


        // --- Number of Players ---
        let num_players_label = gtk::Label::new(Some("Number of Players:"));
        grid_container.attach(&num_players_label, 0, 0, 1, 1);

        let num_players_combo = gtk::ComboBoxText::new();
        for i in 2..=4 {
            num_players_combo.append_text(&i.to_string());
        }
        grid_container.attach(&num_players_combo, 1, 0, 1, 1);
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
        file_path_label.set_ellipsize(pango::EllipsizeMode::Start);
        grid_container.attach(&file_path_label, 1, 2, 1, 1);
         gui_state.borrow_mut().file_path_label = Some(file_path_label.clone()); // Store reference


        // --- Layout Selection ---
        let layout_label = gtk::Label::new(Some("Split-Screen Layout:"));
        grid_container.attach(&layout_label, 0, 3, 1, 1);

        let layout_box = gtk::Box::new(Orientation::Horizontal, 5);
        grid_container.attach(&layout_box, 1, 3, 1, 1);

        let horizontal_radio = gtk::RadioButton::with_label(None, "Horizontal");
        let vertical_radio = gtk::RadioButton::with_label_from_widget(&horizontal_radio, "Vertical");
        let custom_radio = gtk::RadioButton::with_label_from_widget(&horizontal_radio, "Custom");

        layout_box.append(&horizontal_radio);
        layout_box.append(&vertical_radio);
        layout_box.append(&custom_radio);

         // Store references to layout radios
         gui_state.borrow_mut().layout_radios = vec![horizontal_radio.clone(), vertical_radio.clone(), custom_radio.clone()];
         horizontal_radio.set_active(true); // Default layout


        // --- Input Device Assignment (Dynamic) ---
        let input_assignment_label = gtk::Label::new(Some("Input Assignments:"));
        // Attach this label above the dynamic input fields container
        grid_container.attach(&input_assignment_label, 0, 4, 1, 1);


        // Container for dynamically added input fields
        let input_fields_container = Grid::new();
        input_fields_container.set_row_spacing(5);
        input_fields_container.set_column_spacing(5);
        // Attach this container starting below the input assignment label
        grid_container.attach(&input_fields_container, 1, 4, 1, 4); // Adjust row to 4


        // Store a reference to the input fields container to modify it later
        gui_state.borrow_mut().input_fields_container = Some(input_fields_container.clone());


        // Function to populate input device combo box
        let populate_input_combo = |combo: &gtk::ComboBoxText, available_devices: &[DeviceIdentifier]| {
             combo.remove_all(); // Clear existing items
             combo.append_text("Auto-detect"); // Option for auto-detection
             for device_id in available_devices {
                 combo.append_text(&device_id.name); // Add device names
             }
             combo.set_active(0); // Default to "Auto-detect" (index 0)
        };


        // Function to update the dynamic input fields based on player count
        let gui_state_clone_for_update = Rc::clone(&gui_state);
        let update_input_fields = move |num_players: usize| {
            info!("Updating input fields for {} players.", num_players);
            let mut state = gui_state_clone_for_update.borrow_mut();
            let container = state.input_fields_container.as_ref().expect("Input fields container not set");
            let available_devices = &state.available_input_devices;

            // Remove existing input fields
            for child in container.children() {
                container.remove(&child);
            }
            state.input_combos.clear(); // Clear stored combo box references

            // Add new input fields based on the number of players
            for i in 0..num_players {
                let player_label = gtk::Label::new(Some(&format!("Player {}:", i + 1)));
                container.attach(&player_label, 0, i as i32, 1, 1);

                let input_combo = gtk::ComboBoxText::new();
                populate_input_combo(&input_combo, available_devices);
                container.attach(&input_combo, 1, i as i32, 1, 1);

                state.input_combos.push(input_combo); // Store reference
            }
            container.show_all(); // Show the new widgets
            info!("Input fields updated.");
        };

        // Connect signal to "Number of Players" combo box to update input fields dynamically
        num_players_combo.connect_changed(move |combo| {
             if let Some(player_count_str) = combo.get_active_text() {
                 if let Ok(num_players) = player_count_str.parse::<usize>() {
                      // Ensure num_players is within a reasonable range (e.g., 1-4)
                     if num_players > 0 && num_players <= 4 { // Adjust max players as needed
                         update_input_fields(num_players);
                     } else {
                         warn!("Invalid number of players selected: {}. Must be between 1 and 4.", num_players);
                         // TODO: Show a warning dialog to the user
                     }
                 } else {
                      warn!("Failed to parse number of players from combo box text: {:?}", player_count_str);
                     // TODO: Show a warning dialog
                 }
             }
        });


        // --- Control Buttons ---
        let buttons_box = gtk::Box::new(Orientation::Horizontal, 10);
        grid_container.attach(&buttons_box, 0, 9, 2, 1); // Adjust row to 9
        buttons_box.set_halign(Align::End);

        let launch_button = gtk::Button::with_label("Launch Game");
        let cancel_button = gtk::Button::with_label("Cancel");

        buttons_box.append(&cancel_button);
        buttons_box.append(&launch_button);


        // --- Event Handling ---

        // Select Game Executable Button
        let window_clone_for_file_dialog = window.clone(); // Clone window for the closure
        let file_path_label_clone_for_file_dialog = file_path_label.clone();
        select_button.connect_clicked(move |_| {
            let window = &window_clone_for_file_dialog; // Use the cloned window reference
            let dialog = gtk::FileChooserDialog::builder()
                .title("Select Game Executable")
                .action(gtk::FileChooserAction::Open)
                .modal(true)
                .transient_for(window)
                .build();

            let file_path_label_clone = file_path_label_clone_for_file_dialog.clone();
            dialog.add_button("Open", gtk::ResponseType::Accept);
            dialog.add_button("Cancel", gtk::ResponseType::Cancel);

            dialog.connect_response(move |dialog, response| {
                if response == gtk::ResponseType::Accept {
                    if let Some(file) = dialog.file() {
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
        let gui_state_clone_launch = Rc::clone(&gui_state);
        let initial_config_clone_for_launch = initial_config.clone();

        launch_button.connect_clicked(move |_| {
            let state = gui_state_clone_launch.borrow();

            // Collect data from widgets
            let file_path_str = state.file_path_label.as_ref().unwrap().get_text().to_string();
            if file_path_str.is_empty() {
                 warn!("Game executable path not selected. Cannot launch.");
                 // TODO: Show an error dialog to the user
                 return;
            }
            let file_path = PathBuf::from(file_path_str);
            if !file_path.exists() {
                 warn!("Game executable file not found: {}", file_path.display());
                 // TODO: Show an error dialog to the user
                 return;
            }
            if !file_path.is_file() {
                 warn!("Selected path is not a file: {}", file_path.display());
                 // TODO: Show an error dialog to the user
                 return;
            }


            let player_count_str = state.num_players_combo.as_ref().unwrap().get_active_text().unwrap_or_else(|| "2".to_string());
            let player_count = player_count_str.parse::<usize>().unwrap_or(2);


            let mut input_assignments: Vec<String> = Vec::new();
             for combo in &state.input_combos {
                 input_assignments.push(combo.get_active_text().unwrap_or_else(|| "Auto-detect".to_string())); // Default to Auto-detect if unselected
             }
             input_assignments.truncate(player_count);


            let layout_option = if state.layout_radios[0].get_active() { // Horizontal
                "horizontal"
            } else if state.layout_radios[1].get_active() { // Vertical
                "vertical"
            } else { // Custom
                "custom"
            };
            let layout = Layout::from(layout_option);


            let profile_name = state.profile_name_entry.as_ref().unwrap().get_text().to_string();
             // TODO: Use profile_name for saving/loading config profiles


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

            // Prepare input device names for run_core_logic
            let input_device_names_for_core: Vec<&str> = input_assignments.iter().map(|s| s.as_str()).collect();


            // Trigger the core application launch logic in a separate thread
             // Disable launch button and show loading indicator while launching
             let launch_button_clone = launch_button.clone();
             launch_button_clone.set_sensitive(false);
             // TODO: Add a loading indicator (e.g., a Spinner or progress bar)

             let file_path_clone = file_path.clone();
             let initial_config_clone_for_thread = initial_config_clone_for_launch.clone();
             let input_device_names_clone_for_thread: Vec<String> = input_assignments.clone(); // Clone Strings for the thread


             thread::spawn(move || {
                 info!("Launching core logic from GUI thread.");
                 let input_device_names_slice: Vec<&str> = input_device_names_clone_for_thread.iter().map(|s| s.as_str()).collect();

                 let core_result = run_core_logic(
                    &file_path_clone,
                    player_count,
                    &input_device_names_slice,
                    layout,
                    use_proton,
                    &initial_config_clone_for_thread,
                 );

                 // Use glib::idle_add_local or glib::MainContext::default().spawn_local
                 // to update the GUI from the background thread.
                 glib::MainContext::default().spawn_local(async move {
                      // Re-enable the launch button and hide loading indicator
                      launch_button_clone.set_sensitive(true);
                       // TODO: Hide loading indicator

                     match core_result {
                         Ok(_) => info!("Core application logic completed successfully in thread."),
                         Err(e) => {
                            error!("Core application logic failed in thread: {}", e);
                            // TODO: Display this error to the user in the GUI (e.g., a dialog box)
                             show_error_dialog(&state.main_window.as_ref().expect("Main window not set"), "Launch Failed", &format!("Failed to launch game: {}", e));
                         }
                     }
                 });

                 info!("Core logic thread finished.");
             });

        });


        // Cancel Button
        let window_clone_for_cancel = window.clone();
        cancel_button.connect_clicked(move |_| {
            info!("Cancel button clicked. Closing window.");
            // TODO: Implement graceful shutdown of background threads before closing
            window_clone_for_cancel.close();
        });


        // Initial update of input fields based on the default player count
        // and populate widgets with initial config
        let initial_player_count_str = num_players_combo.get_active_text().unwrap_or_else(|| "2".to_string());
        let initial_player_count = initial_player_count_str.parse::<usize>().unwrap_or(2);
         update_input_fields(initial_player_count);

         // TODO: Populate other GUI widgets with values from initial_config


        window.present();
    });

    application.run();

    Ok(())
}


// Helper function to show an error dialog in the GUI
fn show_error_dialog(parent_window: &ApplicationWindow, title: &str, message: &str) {
    let dialog = gtk::MessageDialog::new(
        Some(parent_window),
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Error,
        gtk::ButtonsType::Close,
        message,
    );
    dialog.set_title(Some(title));
    dialog.connect_response(|dialog, _| dialog.close());
    dialog.show();
}

// Helper function to populate input device combo box
// Moved inside run_gui as a local function or closure
/*
fn populate_input_combo(combo: &gtk::ComboBoxText, available_devices: &[DeviceIdentifier]) {
    combo.remove_all();
    combo.append_text("Auto-detect");
    for device_id in available_devices {
        combo.append_text(&device_id.name);
    }
    combo.set_active(0);
}
*/

// Helper function to update the dynamic input fields based on player count
// Moved inside run_gui as a local function or closure
/*
fn update_input_fields(num_players: usize, container: &Grid, available_devices: &[DeviceIdentifier], gui_state: Rc<RefCell<GuiState>>) {
    // ... implementation ...
}
*/

// Note: This file should likely be in src/gui.rs or similar.
