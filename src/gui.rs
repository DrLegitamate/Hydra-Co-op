use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Grid};
use crate::input_mux::InputMux;
use log::info;

/// Initializes GTK and sets up the main application window with a basic container.
fn build_ui() {
    // Initialize GTK
    gtk::init().expect("Failed to initialize GTK");

    // Create a new GTK application
    let app = Application::new(
        Some("com.example.split_screen_launcher"),
        Default::default(),
    );

    // Connect the activate signal of the application to the build_ui function
    app.connect_activate(|app| {
        // Create the main application window
        let window = ApplicationWindow::new(app);
        window.set_title("Split-Screen Launcher Configuration");
        window.set_default_size(1280, 720);

        // Create a grid container to organize the widgets
        let grid_container = Grid::new();
        grid_container.set_row_spacing(10);
        grid_container.set_column_spacing(10);
        window.add(&grid_container);

        // Create a label to indicate the number of players
        let num_players_label = gtk::Label::new(Some("Number of Players"));
        grid_container.attach(&num_players_label, 0, 0, 1, 1);

        // Create a drop-down menu for selecting the number of players
        let num_players_combo = gtk::ComboBoxText::new();
        num_players_combo.append_text("2");
        num_players_combo.append_text("3");
        num_players_combo.append_text("4");
        grid_container.attach(&num_players_combo, 1, 0, 1, 1);

        // Create a text entry widget for the profile name
        let profile_name_label = gtk::Label::new(Some("Profile Name"));
        grid_container.attach(&profile_name_label, 0, 1, 1, 1);

        // This is the only field where manual typing is expected
        let profile_name_entry = gtk::Entry::new();
        grid_container.attach(&profile_name_entry, 1, 1, 1, 1);

        // Add a button to select the game executable
        let select_button = gtk::Button::with_label("Select Game Executable");
        grid_container.attach(&select_button, 0, 2, 1, 1);

        // Create a label to display the selected file path
        let file_path_label = gtk::Label::new(None);
        grid_container.attach(&file_path_label, 1, 2, 1, 1);

        // Add a label to indicate the split-screen layout selection
        let layout_label = gtk::Label::new(Some("Split-Screen Layout"));
        grid_container.attach(&layout_label, 0, 3, 1, 1);

        // Create a grid to hold the layout options
        let layout_grid = Grid::new();
        layout_grid.set_row_spacing(5);
        layout_grid.set_column_spacing(5);
        grid_container.attach(&layout_grid, 1, 3, 1, 1);

        // Create radio buttons for the layout options
        let horizontal_radio = gtk::RadioButton::with_label(None, "Horizontal");
        let vertical_radio = gtk::RadioButton::with_label_from_widget(&horizontal_radio, "Vertical");
        let custom_radio = gtk::RadioButton::with_label_from_widget(&horizontal_radio, "Custom");

        // Add the radio buttons to the grid with labels
        layout_grid.attach(&horizontal_radio, 0, 0, 1, 1);
        layout_grid.attach(&gtk::Label::new(Some("Horizontal")), 0, 1, 1, 1);
        layout_grid.attach(&vertical_radio, 1, 0, 1, 1);
        layout_grid.attach(&gtk::Label::new(Some("Vertical")), 1, 1, 1, 1);
        layout_grid.attach(&custom_radio, 2, 0, 1, 1);
        layout_grid.attach(&gtk::Label::new(Some("Custom")), 2, 1, 1, 1);

        // Inline comment explaining the purpose of the layout options
        // These options let users select the window arrangement without manual configuration

        // Add a label to indicate the input device selection for Player 1
        let player1_input_label = gtk::Label::new(Some("Player 1 Input"));
        grid_container.attach(&player1_input_label, 0, 4, 1, 1);

        // Create a drop-down menu for selecting the input device for Player 1
        let player1_input_combo = gtk::ComboBoxText::new();
        grid_container.attach(&player1_input_combo, 1, 4, 1, 1);

        // Add a label to indicate the input device selection for Player 2
        let player2_input_label = gtk::Label::new(Some("Player 2 Input"));
        grid_container.attach(&player2_input_label, 0, 5, 1, 1);

        // Create a drop-down menu for selecting the input device for Player 2
        let player2_input_combo = gtk::ComboBoxText::new();
        grid_container.attach(&player2_input_combo, 1, 5, 1, 1);

        // Add a label to indicate the input device selection for Player 3
        let player3_input_label = gtk::Label::new(Some("Player 3 Input"));
        grid_container.attach(&player3_input_label, 0, 6, 1, 1);

        // Create a drop-down menu for selecting the input device for Player 3
        let player3_input_combo = gtk::ComboBoxText::new();
        grid_container.attach(&player3_input_combo, 1, 6, 1, 1);

        // Add a label to indicate the input device selection for Player 4
        let player4_input_label = gtk::Label::new(Some("Player 4 Input"));
        grid_container.attach(&player4_input_label, 0, 7, 1, 1);

        // Create a drop-down menu for selecting the input device for Player 4
        let player4_input_combo = gtk::ComboBoxText::new();
        grid_container.attach(&player4_input_combo, 1, 7, 1, 1);

        // Enumerate input devices and populate the drop-down menus
        let mut input_mux = InputMux::new();
        if let Err(e) = input_mux.enumerate_devices() {
            eprintln!("Error enumerating devices: {}", e);
        } else {
            for (path, device) in input_mux.devices.iter() {
                let name = device.name().unwrap_or_else(|| "Unknown".to_string());
                player1_input_combo.append_text(&name);
                player2_input_combo.append_text(&name);
                player3_input_combo.append_text(&name);
                player4_input_combo.append_text(&name);
            }
        }

        // Add two buttons labeled 'Save Settings' and 'Cancel'
        let save_button = gtk::Button::with_label("Save Settings");
        let cancel_button = gtk::Button::with_label("Cancel");
        grid_container.attach(&save_button, 0, 8, 1, 1);
        grid_container.attach(&cancel_button, 1, 8, 1, 1);

        // Inline comments explaining the function of each button
        // 'Save Settings' button: Collects all user selections and prints them to the console
        // 'Cancel' button: Closes the configuration window

        // Connect the 'Save Settings' button click signal to the signal handler
        save_button.connect_clicked(move |_| {
            // Collect all user selections
            let file_path = file_path_label.get_text().to_string();
            let player_count = num_players_combo.get_active_text().unwrap_or_else(|| "".to_string());
            let input_assignments = vec![
                player1_input_combo.get_active_text().unwrap_or_else(|| "".to_string()),
                player2_input_combo.get_active_text().unwrap_or_else(|| "".to_string()),
                player3_input_combo.get_active_text().unwrap_or_else(|| "".to_string()),
                player4_input_combo.get_active_text().unwrap_or_else(|| "".to_string()),
            ];
            let layout_option = if horizontal_radio.get_active() {
                "Horizontal"
            } else if vertical_radio.get_active() {
                "Vertical"
            } else {
                "Custom"
            };
            let profile_name = profile_name_entry.get_text().to_string();

            // Print the collected selections to the console
            info!("File Path: {}", file_path);
            info!("Player Count: {}", player_count);
            info!("Input Assignments: {:?}", input_assignments);
            info!("Layout Option: {}", layout_option);
            info!("Profile Name: {}", profile_name);
        });

        // Connect the 'Cancel' button click signal to the signal handler
        cancel_button.connect_clicked(move |_| {
            // Close the configuration window
            window.close();
        });

        // Connect the button click signal to open a file chooser dialog
        select_button.connect_clicked(move |_| {
            let dialog = gtk::FileChooserDialog::new(
                Some("Select Game Executable"),
                Some(&window),
                gtk::FileChooserAction::Open,
                &[
                    ("Open", gtk::ResponseType::Accept),
                    ("Cancel", gtk::ResponseType::Cancel),
                ],
            );

            if dialog.run() == gtk::ResponseType::Accept {
                if let Some(filename) = dialog.get_filename() {
                    file_path_label.set_text(&filename.to_string_lossy());
                }
            }

            dialog.close();
        });

        // Show all widgets in the window
        window.show_all();
    });

    // Run the application
    app.run();
}
