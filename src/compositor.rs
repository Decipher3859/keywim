use std::sync::Arc;
use std::thread;
use std::time::Duration;

use smithay::reexports::wayland_server::{Display, ListeningSocket};

pub fn initialize() {
    let mut display = Display::<()>::new().expect("Wayland-Display konnte nicht erstelle werden.");

    let socket = ListeningSocket::bind_auto("wayland", 1..32)
        .expect("Wayland-Socket konnte nicht erstellt werden.");
    println!(
        "Wayland-Socket: {}",
        socket.socket_name().unwrap().to_string_lossy()
    );

    loop {
        if let Some(stream) = socket
            .accept()
            .expect("Fehler beim Annehmen einer Verbindung")
        {
            display
                .handle()
                .insert_client(stream, Arc::new(()))
                .expect("Wayland-Client konnte nicht registriert werden.");

            println!("Wayland-Client verbunden");
        }

        display
            .dispatch_clients(&mut ())
            .expect("Fehler bei der Verarbeitung von Clients");

        display.flush_clients().expect("Fehler beim Senden");

        thread::sleep(Duration::from_millis(10));
    }
}
