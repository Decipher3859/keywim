mod helpers;
mod input;
mod rendering;
pub mod tty;

use calloop::{EventLoop, Interest, Mode, PostAction, generic::Generic};
use helpers::send_frames_surface_tree;
use smithay::{
    backend::{
        input::{InputEvent, KeyboardKeyEvent, ProximityState::In},
        renderer::{
            Color32F, Frame, Renderer,
            element::{
                Kind,
                surface::{WaylandSurfaceRenderElement, render_elements_from_surface_tree},
            },
            gles::GlesRenderer,
            utils::{draw_render_elements, on_commit_buffer_handler},
        },
        winit::{self, WinitEvent},
    },
    delegate_compositor, delegate_seat, delegate_shm, delegate_xdg_shell,
    input::{
        Seat, SeatHandler, SeatState,
        keyboard::{FilterResult, XkbConfig},
    },
    reexports::wayland_server::{
        Client, Display, DisplayHandle, ListeningSocket,
        backend::{ClientData, ClientId, DisconnectReason},
        protocol::{wl_buffer, wl_seat::WlSeat, wl_surface::WlSurface},
    },
    utils::{Rectangle, Serial, Transform},
    wayland::{
        buffer::BufferHandler,
        compositor::{CompositorClientState, CompositorHandler, CompositorState},
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
        },
        shm::{ShmHandler, ShmState},
    },
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

// Connects state with smithay protocol processing
delegate_shm!(KeywimState);
delegate_compositor!(KeywimState);
delegate_seat!(KeywimState);
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
    pub shm_state: ShmState,
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
        let shm_state = ShmState::new::<KeywimState>(display, vec![]);
        Self {
            compositor_state,
            display_handle: display.clone(),
            seat,
            seat_state,
            shm_state,
            windows: Vec::new(),
            xdg_shell_state,
        }
    }
}

impl BufferHandler for KeywimState {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
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

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<KeywimState>(surface);

        for window in &self.windows {
            if window.wl_surface() == surface && !window.is_initial_configure_sent() {
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

impl ShmHandler for KeywimState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl XdgShellHandler for KeywimState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }
    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {}
    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        surface.send_configure();
        self.windows.push(surface.clone());

        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.set_focus(self, Some(surface.wl_surface().clone()), 0.into());
        }
        println!("Fenster registriert: {}", self.windows.len());
    }
    fn grab(&mut self, surface: PopupSurface, _seat: WlSeat, _serial: Serial) {}
    fn reposition_request(
        &mut self,
        surface: PopupSurface,
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
pub fn run_winit() {
    let mut display: Display<KeywimState> =
        Display::new().expect("Wayland-Display konnte nicht erstellt werden.");

    let mut state = KeywimState::new(&display.handle());
    let socket = ListeningSocket::bind_auto("wayland", 1..32)
        .expect("Wayland-Socket konnte nicht erstellt werden.");
    println!(
        "Wayland-Socket: {}",
        socket.socket_name().unwrap().to_string_lossy()
    );
    let socket_name = socket.socket_name().unwrap().to_string_lossy().into_owned();

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

    match crate::autostart::launch_alacritty(&socket_name) {
        Ok(child) => println!("Alacritty gestartet: PID {}", child.id()),
        Err(err) => eprintln!("Alacritty konnte nicht gestartet werden: {err}"),
    }

    let start_time = Instant::now();
    let mut last_report = Instant::now();

    loop {
        let mut close_requested = false;
        winit_event_loop.dispatch_new_events(|event| match event {
            WinitEvent::Input(InputEvent::Keyboard { event }) => {
                if let Some(keyboard) = state.seat.get_keyboard() {
                    keyboard.input::<(), _>(
                        &mut state,
                        event.key_code(),
                        event.state(),
                        0.into(),
                        0,
                        |_, _, _| FilterResult::Forward,
                    );
                }
            }
            WinitEvent::CloseRequested => {
                close_requested = true;
            }
            _ => {}
        });
        if close_requested {
            break;
        }

        let size = backend.window_size();
        let damage = Rectangle::from_size(size);

        for window in &state.windows {
            window.with_pending_state(|pending| {
                pending.size = Some((size.w, size.h).into());
            });
            window.send_pending_configure();
        }

        let render_start = Instant::now();
        {
            let (renderer, mut framebuffer) = backend
                .bind()
                .expect("Framebuffer konnte nicht gebunden werden.");

            let elements = state
                .windows
                .iter()
                .flat_map(|window| {
                    render_elements_from_surface_tree(
                        renderer,
                        window.wl_surface(),
                        (0, 0),
                        1.0,
                        1.0,
                        Kind::Unspecified,
                    )
                })
                .collect::<Vec<WaylandSurfaceRenderElement<GlesRenderer>>>();

            let mut frame = renderer
                .render(&mut framebuffer, size, Transform::Flipped180)
                .expect("Frame konnte nicht erstellt werden.");

            frame
                .clear(Color32F::new(0.0, 0.6, 0.7, 1.0), &[damage])
                .expect("Frame konnte nicht eingefärbt werden.");

            draw_render_elements(&mut frame, 1.0, &elements, &[damage])
                .expect("Client-Surfaces konnten nicht gerendert werden.");

            let _sync = frame
                .finish()
                .expect("Frame konnte nicht abgeschlossen werden.");
        }

        let render_duration = render_start.elapsed();
        let submit_start = Instant::now();
        backend
            .submit(Some(&[damage]))
            .expect("Frame konnte nicht angezeigt werden.");

        let submit_duration = submit_start.elapsed();
        if last_report.elapsed() >= Duration::from_secs(1) {
            println!(
                "Gesamt: {:?} | Submit: {:?}",
                render_duration, submit_duration
            );
            last_report = Instant::now();
        }

        for window in &state.windows {
            send_frames_surface_tree(window.wl_surface(), start_time.elapsed().as_millis() as u32);
        }

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
