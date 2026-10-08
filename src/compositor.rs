use calloop::{EventLoop, Interest, Mode, PostAction, generic::Generic};
use smithay::{
    backend::{
        renderer::{Color32F, Frame, Renderer, gles::GlesRenderer},
        winit,
    },
    delegate_compositor, delegate_seat, delegate_xdg_shell,
    input::{Seat, SeatHandler, SeatState, keyboard::XkbConfig},
    reexports::wayland_server::{
        Client, Display, DisplayHandle, ListeningSocket,
        backend::{ClientData, ClientId, DisconnectReason},
        protocol::{wl_seat::WlSeat, wl_surface::WlSurface},
    },
    utils::{Rectangle, Serial, Transform},
    wayland::{
        compositor::{CompositorClientState, CompositorHandler, CompositorState},
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
        },
    },
};
use std::{sync::Arc, time::Duration};

delegate_seat!(KeywimState);
delegate_compositor!(KeywimState);
// Connects state with smithay protocol processing
delegate_xdg_shell!(KeywimState);

pub struct KeywimState {
    // Handles Wayland Surfaces
    pub compositor_state: CompositorState,
    // Accesses the Wayland server
    // Handles new clients
    pub display_handle: DisplayHandle,
    // Handles peripherie
    pub seat: Seat<KeywimState>,
    pub seat_state: SeatState<KeywimState>,
    // Window and popup protocol
    pub xdg_shell_state: XdgShellState,
    // All currently registeres application Window
    pub windows: Vec<ToplevelSurface>,
}

impl KeywimState {
    /// Initializes the Keywim compositors state
    ///
    /// # Arguments
    /// * `display` - Handles the Wayland display
    ///
    /// # Returns
    pub fn new(display: &DisplayHandle) -> Self {
        let compositor_state = CompositorState::new::<KeywimState>(display);
        let xdg_shell_state = XdgShellState::new::<KeywimState>(display);
        let mut seat_state: SeatState<KeywimState> = SeatState::new();
        let mut seat = seat_state.new_wl_seat(display, "seat0");
        seat.add_keyboard(
            XkbConfig {
                layout: "de",
                ..XkbConfig::default()
            },
            200,
            25,
        )
        .expect("Tastatur konnte nicht initialisiert werden.");
        Self {
            compositor_state,
            display_handle: display.clone(),
            seat,
            seat_state,
            windows: Vec::new(),
            xdg_shell_state,
        }
    }
}

impl CompositorHandler for KeywimState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client
            .get_data::<KeywimClientState>()
            .expect("CompositorClientState fehlt.")
            .compositor_state
    }

    fn commit(&mut self, _surface: &WlSurface) {
        for window in &self.windows {
            if window.wl_surface() == _surface && !window.is_initial_configure_sent() {
                window.send_configure();
            }
        }
    }
}
// Connects state with smithay protocol processing

impl SeatHandler for KeywimState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }
    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&WlSurface>) {}
    fn cursor_image(
        &mut self,
        _seat: &Seat<Self>,
        _image: smithay::input::pointer::CursorImageStatus,
    ) {
    }
}

impl XdgShellHandler for KeywimState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }
    fn new_popup(&mut self, _surface: PopupSurface, _positioner: PositionerState) {}
    fn new_toplevel(&mut self, _surface: ToplevelSurface) {
        _surface.send_configure();
        self.windows.push(_surface);
        println!("Fenster registriert: {}", self.windows.len());
    }
    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {}
    fn reposition_request(
        &mut self,
        _surface: PopupSurface,
        _positioner: PositionerState,
        _token: u32,
    ) {
    }
    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        self.windows.retain(|window| window != &surface);
        println!("Fenster verbleibend: {}", self.windows.len());
    }
}
#[derive(Default)]
pub struct KeywimClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for KeywimClientState {
    fn initialized(&self, _client_id: ClientId) {}

    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

pub fn initialize() {
    let mut display: Display<KeywimState> =
        Display::new().expect("Wayland-Display konnte nicht erstellt werden.");

    let mut state = KeywimState::new(&display.handle());
    let socket = ListeningSocket::bind_auto("wayland", 1..32)
        .expect("Wayland-Socket konnte nicht erstellt werden.");
    println!(
        "Wayland-Socket: {}",
        socket.socket_name().unwrap().to_string_lossy()
    );

    let (mut backend, mut winit_event_loop) =
        winit::init::<GlesRenderer>().expect("Winit-Backend konnte nicht initialisiert werden.");

    let mut event_loop: EventLoop<KeywimState> =
        EventLoop::try_new().expect("Calloop konnte nicht erstellt werden.");

    event_loop
        .handle()
        .insert_source(
            Generic::new(socket, Interest::READ, Mode::Level),
            move |_, socket, state| {
                if let Some(stream) = socket
                    .accept()
                    .expect("Fehler beim Annehmen einer Verbindung")
                {
                    state
                        .display_handle
                        .insert_client(
                            stream,
                            Arc::new(KeywimClientState {
                                compositor_state: CompositorClientState::default(),
                            }),
                        )
                        .expect("Wayland-Client konnte nicht registriert werden.");

                    println!("Wayland-Client verbunden");
                }

                Ok(PostAction::Continue)
            },
        )
        .expect("Socket konnte nicht registriert werden.");

    loop {
        winit_event_loop.dispatch_new_events(|event| {
            println!("Winit-Ereignis: {:?}", event);
        });

        let size = backend.window_size();
        let damage = Rectangle::from_size(size);

        {
            let (renderer, mut framebuffer) = backend
                .bind()
                .expect("Framebuffer konnte nicht gebunden werden.");

            let mut frame = renderer
                .render(&mut framebuffer, size, Transform::Normal)
                .expect("Frame konnte nicht erstellt werden.");

            frame
                .clear(Color32F::new(0.0, 0.6, 0.7, 1.0), &[damage])
                .expect("Frame konnte nicht eingefärbt werden.");

            let _sync = frame
                .finish()
                .expect("Frame konnte nicht abgeschlossen werden.");
        }

        backend
            .submit(Some(&[damage]))
            .expect("Frame konnte nicht angezeigt werden.");

        event_loop
            .dispatch(Some(Duration::from_millis(10)), &mut state)
            .expect("Fehler in Calloop");

        display
            .dispatch_clients(&mut state)
            .expect("Wayland-Anfragen konnten nicht verarbeitet werden");

        display
            .flush_clients()
            .expect("Wayland-Antworten konnten nicht gesendet werden.");
    }
}
