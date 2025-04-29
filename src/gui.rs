use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Grid, ComboBoxText, Entry, Button, Label, RadioButton, FileChooserDialog, Align, Orientation, MessageDialog, DialogFlags, MessageType, ButtonsType}; // Import dialog types
use crate::input_mux::{InputMux, DeviceIdentifier, InputAssignment}; // Import InputAssignment
use log::{info, error, warn, debug};
use std::rc::Rc;
use std::cell::RefCell;
use std::path::PathBuf;
use crate::config::{Config, ConfigError}; // Import ConfigError
use crate::window_manager::Layout;
use std::collections::HashMap;
use crate::run_core_logic;
use std::thread::{self, JoinHandle}; // Import JoinHandle
use std::error::Error; // Import Error trait for boxed errors
use std::sync::{Arc, Mutex}; // Import Arc and Mutex for shared mutable state across threads


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
    input_fields_container: Option<Grid>, // Store reference to the container Grid
    main_window: Option<ApplicationWindow>, // Store reference to the main window
    initial_config: Config, // Store initial config for persistence and defaults

    // Store instances of background services spawned by the core logic thread
    // Use Arc<Mutex<>> to allow safe shared access across threads (GUI thread and shutdown handler)
    background_services: Arc<Mutex<Option<(NetEmulator, InputMux)>>>, // Store optional tuple of services

    // Store the JoinHandle of the core logic thread
    core_logic_thread: Arc<Mutex<Option<JoinHandle<Result<(NetEmulator, InputMux), Box<dyn Error>>>>>>,

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

    // Share the background services state and thread handle using Arc and Mutex
    let background_services_state = Arc::new(Mutex::new(None));
    let core_logic_thread_handle = Arc::new(Mutex::new(None));
     gui_state.borrow_mut().background_services = Arc::clone(&background_services_state);
     gui_state.borrow_mut().core_logic_thread = Arc::clone(&core_logic_thread_handle);


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


        // --- Input Device Assignment (Dynamic Placeholder) ---
        let input_assignment_label = gtk::Label::new(Some("Input Assignments:"));
        grid_container.attach(&input_assignment_label, 0, 4, 1, 1);

        let input_fields_container = Grid::new();
        input_fields_container.set_row_spacing(5);
        input_fields_container.set_column_spacing(5);
        grid_container.attach(&input_fields_container, 1, 4, 1, 4);

        gui_state.borrow_mut().input_fields_container = Some(input_fields_container.clone());


        // Function to populate input device combo box
        let populate_input_combo = |combo: &gtk::ComboBoxText, available_devices: &[DeviceIdentifier]| {
             combo.remove_all();
             combo.append_text("Auto-detect");
             // Append device names to the combo box, storing the DeviceIdentifier string representation as the ID
             for device_id in available_devices {
                 combo.append(&serde_json::to_string(device_id).expect("Failed to serialize device ID").as_str(), &device_id.name);
             }
             combo.set_active_id(Some("Auto-detect")); // Default to "Auto-detect"
        };


        // Function to update the dynamic input fields based on player count
        let gui_state_clone_for_update = Rc::clone(&gui_state);
        let update_input_fields = move |num_players: usize| {
            info!("Updating input fields for {} players.", num_players);
            let mut state = gui_state_clone_for_update.borrow_mut();
            let container = state.input_fields_container.as_ref().expect("Input fields container not set");
            let available_devices = &state.available_input_devices;

            for child in container.children() {
                container.remove(&child);
            }
            state.input_combos.clear();

            for i in 0..num_players {
                let player_label = gtk::Label::new(Some(&format!("Player {}:", i + 1)));
                container.attach(&player_label, 0, i as i32, 1, 1);

                let input_combo = gtk::ComboBoxText::new();
                populate_input_combo(&input_combo, available_devices);
                container.attach(&input_combo, 1, i as i32, 1, 1);

                state.input_combos.push(input_combo);
            }
            container.show_all();
            info!("Input fields updated.");
        };

        // Connect signal to "Number of Players" combo box
        let gui_state_clone_fields = Rc::clone(&gui_state);
        num_players_combo.connect_changed(move |combo| {
             if let Some(player_count_str) = combo.get_active_text() {
                 if let Ok(num_players) = player_count_str.parse::<usize>() {
                     if num_players > 0 && num_players <= 4 {
                         update_input_fields(num_players);
                     } else {
                         warn!("Invalid number of players selected: {}. Must be between 1 and 4.", num_players);
                         show_warning_dialog(&gui_state_clone_fields.borrow().main_window.as_ref().expect("Main window not set"), "Invalid Player Count", &format!("Please select a number of players between 1 and 4."));
                     }
                 } else {
                      warn!("Failed to parse number of players from combo box text: {:?}", player_count_str);
                      show_warning_dialog(&gui_state_clone_fields.borrow().main_window.as_ref().expect("Main window not set"), "Invalid Input", "Failed to parse the selected number of players.");
                 }
             }
        });

        // --- Control Buttons ---
        let buttons_box = gtk::Box::new(Orientation::Horizontal, 10);
        grid_container.attach(&buttons_box, 0, 9, 2, 1);
        buttons_box.set_halign(Align::End);

        let save_button = gtk::Button::with_label("Save Settings"); // Added Save button back
        let launch_button = gtk::Button::with_label("Launch Game");
        let cancel_button = gtk::Button::with_label("Cancel");

        buttons_box.append(&save_button); // Add Save button to box
        buttons_box.append(&cancel_button);
        buttons_box.append(&launch_button);


        // --- Event Handling ---

        // Select Game Executable Button
        let window_clone_for_file_dialog = window.clone();
        let file_path_label_clone_for_file_dialog = file_path_label.clone();
        select_button.connect_clicked(move |_| {
            let window = &window_clone_for_file_dialog;
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

        // Save Settings Button
        let gui_state_clone_save = Rc::clone(&gui_state);
        save_button.connect_clicked(move |_| {
            let state = gui_state_clone_save.borrow();
            let main_window = state.main_window.as_ref().expect("Main window not set for saving");

            // Collect data to save
            let file_path_str = state.file_path_label.as_ref().unwrap().get_text().to_string();
             let game_paths = if file_path_str.is_empty() {
                 vec![]
             } else {
                 vec![PathBuf::from(file_path_str)] // Store as Vec<PathBuf>
             };

             // Collect input assignments (names/auto)
             let mut input_mappings: Vec<String> = Vec::new();
             for combo in &state.input_combos {
                 // Save the active ID (which is "Auto-detect" or the serialized DeviceIdentifier string)
                 input_mappings.push(combo.get_active_id().unwrap_or_else(|| "Auto-detect".to_string()));
             }
             // Optionally truncate input mappings to match the *current* player count if saving
             // let player_count_str = state.num_players_combo.as_ref().unwrap().get_active_text().unwrap_or_else(|| "2".to_string());
             // let player_count = player_count_str.parse::<usize>().unwrap_or(2);
             // input_mappings.truncate(player_count);


            let layout_option = if state.layout_radios[0].get_active() {
                "horizontal"
            } else if state.layout_radios[1].get_active() {
                "vertical"
            } else {
                "custom"
            };
            let window_layout = layout_option.to_string(); // Store as String


             // TODO: Collect network_ports and other future config options from GUI controls


             // Create a new Config struct
            let new_config = Config {
                game_paths,
                input_mappings, // Save the collected mappings (names/serialized IDs)
                window_layout,
                network_ports: state.initial_config.network_ports.clone(), // Placeholder: use initial config ports
                // TODO: Collect network_ports from GUI if added
            };

            // Save the config to the file
            let config_path_str = env::var("CONFIG_PATH").unwrap_or_else(|_| "config.toml".to_string());
            let config_path = PathBuf::from(config_path_str);

            match new_config.save(&config_path) {
                Ok(_) => {
                    info!("Configuration saved successfully to {}", config_path.display());
                    show_info_dialog(main_window, "Settings Saved", &format!("Configuration saved successfully to {}", config_path.display()));
                }
                Err(e) => {
                    error!("Failed to save configuration to {}: {}", config_path.display(), e);
                    show_error_dialog(main_window, "Save Failed", &format!("Failed to save configuration: {}", e));
                }
            }
        });


        // Launch Game Button
        let gui_state_clone_launch = Rc::clone(&gui_state);

        launch_button.connect_clicked(move |_| {
            let state = gui_state_clone_launch.borrow();
            let main_window = state.main_window.as_ref().expect("Main window not set for launch");

            // --- Collect Data from Widgets ---
            let file_path_str = state.file_path_label.as_ref().unwrap().get_text().to_string();
            if file_path_str.is_empty() {
                 warn!("Game executable path not selected. Cannot launch.");
                 show_warning_dialog(main_window, "Launch Error", "Please select a game executable.");
                 return;
            }
            let file_path = PathBuf::from(file_path_str);
            if !file_path.exists() {
                 warn!("Game executable file not found: {}", file_path.display());
                 show_error_dialog(main_window, "Launch Error", &format!("Game executable file not found: {}", file_path.display()));
                 return;
            }
            if !file_path.is_file() {
                 warn!("Selected path is not a file: {}", file_path.display());
                 show_error_dialog(main_window, "Launch Error", &format!("Selected path is not a file: {}", file_path.display()));
                 return;
            }


            let player_count_str = state.num_players_combo.as_ref().unwrap().get_active_text().unwrap_or_else(|| "2".to_string());
            let player_count = player_count_str.parse::<usize>().unwrap_or(2);


            let mut input_assignments_for_core: Vec<(usize, InputAssignment)> = Vec::new();
             // Map collected combo box selections to InputAssignment enum
             for (i, combo) in state.input_combos.iter().enumerate() {
                 if i >= player_count { break; } // Only process up to the selected player count

                 let active_id = combo.get_active_id().unwrap_or_else(|| "Auto-detect".to_string()); // Get the stored ID

                 let assignment = if active_id == "Auto-detect" {
                     info!("Player {}: Input assigned to Auto-detect.", i + 1);
                     InputAssignment::AutoDetect
                 } else {
                     // Attempt to deserialize the stored DeviceIdentifier string
                     match serde_json::from_str::<DeviceIdentifier>(&active_id) {
                         Ok(device_id) => {
                              // Check if the device exists among available devices (optional but good practice)
                              if state.available_input_devices.contains(&device_id) {
                                  info!("Player {}: Input assigned to device '{:?}'.", i + 1, device_id);
                                   InputAssignment::Device(device_id)
                              } else {
                                  warn!("Player {}: Assigned device '{:?}' not found among available devices. Assigning None.", i + 1, device_id);
                                   // TODO: Show a warning dialog
                                   InputAssignment::None
                              }
                         }
                         Err(e) => {
                             error!("Player {}: Failed to deserialize DeviceIdentifier from active ID '{}': {}", i + 1, active_id, e);
                              // TODO: Show a warning/error dialog
                             InputAssignment::None // Assign None on deserialization error
                         }
                     }
                 };
                 input_assignments_for_core.push((i, assignment));
             }
             // If fewer assignments collected than players, add None for the rest
             while input_assignments_for_core.len() < player_count {
                  let i = input_assignments_for_core.len();
                  info!("Player {}: No input assignment specified. Assigning None.", i + 1);
                  input_assignments_for_core.push((i, InputAssignment::None));
             }
             debug!("Input assignments for core logic: {:?}", input_assignments_for_core);


            let layout_option = if state.layout_radios[0].get_active() {
                "horizontal"
            } else if state.layout_radios[1].get_active() {
                "vertical"
            } else {
                "custom"
            };
            let layout = Layout::from(layout_option);


            let profile_name = state.profile_name_entry.as_ref().unwrap().get_text().to_string();
             // TODO: Use profile_name for saving/loading config profiles


            // TODO: Implement logic to get the use_proton flag from the GUI (e.g., a checkbox)
            let use_proton = false; // Placeholder - needs a GUI control


            info!("--- Triggering Core Logic from GUI ---");
            // ... Log collected settings ...
            info!("-----------------------------------------");


            // Trigger the core application launch logic in a separate thread
            // Disable launch button and show loading indicator while launching
             let launch_button_clone = launch_button.clone();
             launch_button_clone.set_sensitive(false);
             // TODO: Add a loading indicator (e.g., a Spinner or progress bar)

             let file_path_clone = file_path.clone();
             let initial_config_clone_for_thread = initial_config_clone_for_launch.clone();
             // Clone input_assignments_for_core for the thread
             let input_assignments_clone_for_thread = input_assignments_for_core.clone();

            // Acquire mutex lock on the thread handle BEFORE spawning
            let core_logic_thread_handle_clone = Arc::clone(&state.core_logic_thread);
             let mut thread_handle_lock = core_logic_thread_handle_clone.lock().expect("Failed to lock core_logic_thread handle");
             // Ensure no thread is already running
             if thread_handle_lock.is_some() {
                 warn!("Core logic thread is already running. Cannot launch again.");
                  show_warning_dialog(main_window, "Launch In Progress", "Core launch logic is already running.");
                 launch_button_clone.set_sensitive(true); // Re-enable if we didn't launch
                  return;
             }


             let background_services_state_clone = Arc::clone(&state.background_services); // Clone for the thread


             let join_handle = thread::spawn(move || {
                 info!("Launching core logic from GUI thread.");

                 let core_result = run_core_logic(
                    &file_path_clone,
                    player_count,
                    &input_assignments_clone_for_thread, // Pass the Assignment vector
                    layout,
                    use_proton,
                    &initial_config_clone_for_thread,
                 );

                 // Store the returned background services instances if successful
                 if let Ok((net_emu, input_mux)) = core_result {
                      info!("Core logic returned background service instances.");
                      let mut services_lock = background_services_state_clone.lock().expect("Failed to lock background_services state");
                      *services_lock = Some((net_emu, input_mux)); // Store the instances
                      info!("Background service instances stored.");
                 } else {
                      error!("Core logic returned an error, no background service instances to store.");
                 }


                 // Use glib::idle_add_local or glib::MainContext::default().spawn_local
                 // to update the GUI from the background thread.
                 glib::MainContext::default().spawn_local(async move {
                      // Re-enable the launch button and hide loading indicator
                      launch_button_clone.set_sensitive(true);
                       // TODO: Hide loading indicator

                     // Check the result of the core logic
                     match core_result {
                         Ok(_) => info!("Core application logic completed successfully in thread."),
                         Err(e) => {
                            error!("Core application logic failed in thread: {}", e);
                            show_error_dialog(&state.main_window.as_ref().expect("Main window not set"), "Launch Failed", &format!("Failed to launch game: {}", e));
                         }
                     }
                     // Clear the thread handle after it finishes
                     let core_logic_thread_handle_clone_inner = Arc::clone(&core_logic_thread_handle); // Need to clone again for this closure
                      let mut thread_handle_lock_inner = core_logic_thread_handle_clone_inner.lock().expect("Failed to lock core_logic_thread handle for clearing");
                      *thread_handle_lock_inner = None; // Clear the handle

                 });

                 // The thread returns the result of run_core_logic
                 core_result
             });

            // Store the join handle
             *thread_handle_lock = Some(join_handle); // Store the join handle in the shared state


        });


        // Cancel Button and Window Close Request
        let window_clone_for_cancel = window.clone();
         let gui_state_clone_for_shutdown = Rc::clone(&gui_state); // Clone for the shutdown handler

        cancel_button.connect_clicked(move |_| {
            info!("Cancel button clicked.");
            // Trigger window close which will then trigger the close_request signal
            window_clone_for_cancel.close();
        });

         // Connect the close_request signal to handle graceful shutdown
         window.connect_close_request(move |win| {
             info!("Window close requested.");
             let state = gui_state_clone_for_shutdown.borrow();

             // Check if core logic thread is running
             let mut thread_handle_lock = state.core_logic_thread.lock().expect("Failed to lock core_logic_thread handle during shutdown");
             if let Some(thread_handle) = thread_handle_lock.take() { // Take the handle to signal stopping
                 info!("Core logic thread is running. Signaling for shutdown.");

                 // Signal background services to stop
                 let mut services_lock = state.background_services.lock().expect("Failed to lock background_services during shutdown");
                 if let Some((net_emu, input_mux)) = services_lock.take() { // Take the services to stop them
                      info!("Stopping background NetEmulator and InputMux.");
                     // Spawn a new thread to stop services and join the core logic thread
                      // to avoid blocking the GTK main loop during shutdown.
                     thread::spawn(move || {
                         info!("Shutdown thread started. Stopping background services...");
                         if let Err(e) = net_emu.stop_relay() {
                             error!("Error stopping network relay during shutdown thread: {}", e);
                         } else {
                             info!("Network relay stopped in shutdown thread.");
                         }
                          // Note: InputMux stop_capture should also be called here
                          // but needs to be implemented. Assume it exists for now.
                         if let Err(e) = input_mux.stop_capture() { // Assuming stop_capture exists
                             error!("Error stopping input capture during shutdown thread: {}", e);
                         } else {
                             info!("Input capture stopped in shutdown thread.");
                         }

                         info!("Waiting for core logic thread to join...");
                         // Wait for the core logic thread to finish after signaling stops
                         match thread_handle.join() {
                             Ok(thread_result) => {
                                 if let Err(e) = thread_result {
                                     error!("Core logic thread finished with error during shutdown join: {}", e);
                                 } else {
                                     info!("Core logic thread joined successfully during shutdown.");
                                 }
                             }
                             Err(e) => error!("Core logic thread panicked during shutdown join: {:?}", e),
                         }
                          info!("Shutdown thread finished.");
                         // At this point, all background threads related to this launch should be stopped.
                         // The main application can now safely exit.
                     });

                      // Inhibit the window close until the shutdown thread finishes
                      // This is complex to manage correctly. A simpler approach might be
                      // to just let the main thread exit and rely on Drop implementations
                      // or signal handling in main.rs.

                     // For now, inhibit the close request and rely on the shutdown thread
                     // to eventually allow the application to exit.
                     // Returning Inhibit(true) tells GTK to not close the window yet.
                     return Inhibit(true); // Inhibit closing while shutting down threads
                 } else {
                      info!("No background services to stop.");
                 }
             } else {
                 info!("Core logic thread was not running or already finished.");
             }

             // Allow the window to close if no core logic thread is running or being shut down
             Inhibit(false) // Allow the window to close
         });


        // Initial update of input fields based on the default player count
        let initial_player_count_str = num_players_combo.get_active_text().unwrap_or_else(|| "2".to_string());
        let initial_player_count = initial_player_count_str.parse::<usize>().unwrap_or(2);
         update_input_fields(initial_player_count);

         // TODO: Populate other GUI widgets with values from initial_config
         // Example: Set selected game executable path if present in config
         if let Some(game_path) = initial_config.game_paths.first() {
              state.file_path_label.as_ref().expect("File path label not set").set_text(&game_path.to_string_lossy());
         }
         // Example: Set selected layout from config
          match initial_config.window_layout.as_str() {
               "horizontal" => state.layout_radios[0].set_active(true),
               "vertical" => state.layout_radios[1].set_active(true),
               "custom" => state.layout_radios[2].set_active(true),
               _ => warn!("Unknown layout in config: {}", initial_config.window_layout),
          }
          // Example: Set number of players from config (if stored and valid)
          // You would need to store player count in config.toml for this.
          // For now, the combo box defaults to 2.


         // TODO: Populate input device combo box selections from config.input_mappings
         // This requires iterating through input_combos and initial_config.input_mappings,
         // finding the corresponding combo box for each instance index, and setting
         // its active ID based on the saved mapping string ("Auto-detect" or serialized DeviceIdentifier).
         let initial_input_mappings = initial_config.input_mappings.clone(); // Clone for closure
         let input_combos_clone_for_config = state.input_combos.clone(); // Clone combo refs

         // Use glib::MainContext::default().spawn_local to safely populate combo selections
         // after the widgets are fully realized.
         glib::MainContext::default().spawn_local(async move {
             // This runs on the main thread after the GUI is likely ready.
             for (i, mapping_str) in initial_input_mappings.iter().enumerate() {
                 // Find the combo box for this instance index
                 if let Some(combo) = input_combos_clone_for_config.get(i) {
                     combo.set_active_id(Some(mapping_str));
                      debug!("Set combo box {} active ID to {}", i, mapping_str);
                 } else {
                      warn!("No input combo box found for instance index {} to load config.", i);
                 }
             }
         });


        window.present();
    });

    // The application.run() call is blocking and runs the GTK main event loop.
    // The application will exit when the main window is closed and all GTK resources are cleaned up.
    application.run();

    Ok(()) // Return Ok on successful application run (after GUI exits)
}


// Helper function to show an error dialog in the GUI
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

// Helper function to show a warning dialog in the GUI
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

// Helper function to show an info dialog in the GUI
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
