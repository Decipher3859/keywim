use std::{
    io,
    process::{Child, Command},
};

pub fn launch_alacritty(socket_name: &str) -> io::Result<Child> {
    Command::new("alacritty")
        .env("WAYLAND_DISPLAY", socket_name)
        .env("WINIT_UNIX_BACKEND", "wayland")
        .spawn()
}
