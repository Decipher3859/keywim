use super::{KeywimClientState, KeywimState, helpers::send_frames_surface_tree};
use crate::{
    monitoring::battery::{BatteryStatus, BatteryWarning, find_battery},
    widgets::rasterization::WidgetRasterizer,
};
use calloop::EventLoop;
use smithay::{
    backend::{
        allocator::{
            Fourcc,
            dmabuf::Dmabuf,
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        },
        drm::{DrmDevice, DrmDeviceFd, DrmEvent, GbmBufferedSurface},
        egl::{EGLContext, EGLDisplay},
        input::{Event, InputEvent, KeyState, KeyboardKeyEvent},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            Bind, Color32F, Frame, Renderer, gles::GlesRenderer, utils::draw_render_elements,
        },
        session::{Event as SessionEvent, Session, libseat::LibSeatSession},
        udev::UdevBackend,
    },
    input::keyboard::{FilterResult, keysyms},
    reexports::{
        drm::control::{Device as _, ModeTypeFlags, connector::State},
        input::Libinput,
        rustix::fs::OFlags,
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{Display, ListeningSocket},
    },
    utils::{Rectangle, SERIAL_COUNTER, Transform},
};
use std::{
    cell::{Cell, RefCell},
    fs,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

/// Runs the compositor directly n a TTY.
///
/// Sets up seat-managed graphics and input devices, initializes the Wayland server, and launches
/// Alacritty. Processes client requests, input, rendering, and seat activation changes until
/// Alacritty exits.
///
/// # Errors
/// Returns an error description if setup or runtime processing fails.
pub fn run_tty() -> Result<(), String> {
    // Obtain the session for device access and its event source for activation and deactivation
    // notifications.
    let (mut session, seat_notifier) =
        LibSeatSession::new().map_err(|e| format!("Seat initialization failed: {e}"))?;
    println!("Seat opened: {}", session.seat());

    // Store the notifier in an Option so registration inside the device/connector loops can take
    // ownership once, leaving None to detect repeated attempts.
    let mut seat_notifier = Some(seat_notifier);

    // Discover input devices through udev and open them through current seat session.
    let mut libinput = Libinput::new_with_udev(LibinputSessionInterface::from(session.clone()));

    // Restrict input handling to devices assigned to current seat.
    libinput
        .udev_assign_seat(&session.seat())
        .map_err(|_| "Libinput seat assignment failed".to_string())?;

    // Retain access to the shared libinput context to pause and reactivate the session.
    // Store the backend in an Option so event-loop registration can take ownership once.
    let mut input_backend = Some(LibinputInputBackend::new(libinput.clone()));
    let udev =
        UdevBackend::new(session.seat()).map_err(|e| format!("Udev initialization failed: {e}"))?;

    // Iterate over DRM devices on current seat; only paths are needed.
    for (_, path) in udev.device_list() {
        println!("DRM device: {}", path.display());

        // Open the device through the session for read/write access.
        // Close the file descriptor on exec so child programs do not inherit it.
        let fd = session
            .open(path, OFlags::RDWR | OFlags::CLOEXEC)
            .map_err(|e| format!("DRM open failed: {e}"))?;
        println!("DRM opened: {}", path.display());

        // Wrap the opened DRM device and query its available resource handles.
        let drm = DrmDeviceFd::new(fd.into());
        let resources = drm
            .resource_handles()
            .map_err(|e| format!("DRM resources failed: {e}"))?;

        // Query the device's display connectors and skip those without a connected display.
        for handle in resources.connectors() {
            let connector = drm
                .get_connector(*handle, false)
                .map_err(|e| format!("Connector query failed: {e}"))?;

            println!(
                "{:?}-{}: {:?}",
                connector.interface(),
                connector.interface_id(),
                connector.state()
            );

            //Skip connectors without connected display.
            if connector.state() != State::Connected {
                continue;
            }

            // Use the preferred display mode, falling back to the first available mode.
            let Some(mode) = connector
                .modes()
                .iter()
                .find(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
                .or_else(|| connector.modes().first())
                .copied()
            else {
                // Keep searching other connectors if this display offers no modes.
                eprintln!(
                    "Skipping {:?}-{}: connected display offers no modes",
                    connector.interface(),
                    connector.interface_id()
                );
                continue;
            };
            println!("Selected mode: {:?}", mode);

            // Find a CRTC that can drive this connector through one of its encoders.
            let mut selected_crtc = None;
            for encoder_handle in connector.encoders() {
                let encoder = drm
                    .get_encoder(*encoder_handle)
                    .map_err(|e| format!("Encoder query failed: {e}"))?;

                selected_crtc = resources
                    .filter_crtcs(encoder.possible_crtcs())
                    .first()
                    .copied();
                if selected_crtc.is_some() {
                    break;
                }
            }

            // Keep searching if no CRTC can drive this connector.
            let Some(crtc) = selected_crtc else {
                eprintln!(
                    "Skipping {:?}-{}: no compatible CRTC found",
                    connector.interface(),
                    connector.interface_id()
                );
                continue;
            };
            println!(
                "Selected output: {:?}-{}, CRTC: {:?}, mode: {:?}",
                connector.interface(),
                connector.interface_id(),
                crtc,
                mode,
            );

            // Initialize DRM output management and its envent source.
            let (mut device, drm_notifier) = DrmDevice::new(drm.clone(), false)
                .map_err(|e| format!("DRM initialization failed: {e}"))?;

            // Create an output surface for the sleected CRTC, mode, and connector.
            let surface = device
                .create_surface(crtc, mode, &[*handle])
                .map_err(|e| format!("DRM surface creation failed: {e}"))?;

            println!("DRM surface created");

            let gbm = GbmDevice::new(drm.clone())
                .map_err(|e| format!("GBM initialization failed: {e}"))?;

            println!("GBM initialized");

            let display = unsafe { EGLDisplay::new(gbm.clone()) }
                .map_err(|e| format!("EGK display failed: {e}"))?;
            let context =
                EGLContext::new(&display).map_err(|e| format!("EGL context failed: {e}"))?;
            let mut renderer = unsafe { GlesRenderer::new(context) }
                .map_err(|e| format!("Gles renderer failed: {e}"))?;

            println!("Gles renderer initialized");

            let formats = <GlesRenderer as Bind<Dmabuf>>::supported_formats(&renderer)
                .ok_or_else(|| "No renderer target formats".to_string())?;
            let allocator = GbmAllocator::new(
                gbm.clone(),
                GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
            );

            let mut buffered = GbmBufferedSurface::<_, ()>::new(
                surface,
                allocator,
                &[Fourcc::Xrgb8888, Fourcc::Argb8888],
                formats,
            )
            .map_err(|e| format!("Buffered surface failed: {e}"))?;

            println!("Buffered surface created");
            let (mut buffer, _) = buffered
                .next_buffer()
                .map_err(|e| format!("Next buffer failed: {e}"))?;
            let size = (i32::from(mode.size().0), i32::from(mode.size().1)).into();
            let damage = Rectangle::from_size(size);

            let sync = {
                let mut target = renderer
                    .bind(&mut buffer)
                    .map_err(|e| format!("Bind failed: {e}"))?;
                let mut frame = renderer
                    .render(&mut target, size, Transform::Normal)
                    .map_err(|e| format!("Render failed: {e}"))?;
                frame
                    .clear(Color32F::new(0.0, 0.6, 0.7, 1.0), &[damage])
                    .map_err(|e| format!("Clear failed: {e}"))?;
                frame.finish().map_err(|e| format!("Finish failed: {e}"))?
            };

            buffered
                .queue_buffer(Some(sync), None, ())
                .map_err(|e| format!("Queue failed: {e}"))?;
            println!("Test frame queued");
            let mut event_loop = EventLoop::<KeywimState>::try_new()
                .map_err(|e| format!("Eventloop failed: {e}"))?;

            let mut wayland = Display::<KeywimState>::new()
                .map_err(|e| format!("Wayland dislpay failed: {e}"))?;
            let mut state = KeywimState::new(&wayland.handle());

            // Battery monitoring is optional and must not prevent session startup.
            let battery_path = match find_battery() {
                Ok(path) => path,
                Err(error) => {
                    eprintln!("Battery detection failed: {error}");
                    None
                }
            };

            let mut battery_warning = BatteryWarning::default();
            let mut next_battery_check = Instant::now();

            let socket = ListeningSocket::bind_auto("wayland", 1..32)
                .map_err(|e| format!("Wayland socket failed: {e}"))?;
            let socket_name = socket.socket_name().unwrap().to_string_lossy();
            let mut terminal = crate::autostart::launch_alacritty(&socket_name)
                .map_err(|e| format!("Alacritty failed: {e}"))?;

            let seat_events = Rc::new(RefCell::new(Vec::<SessionEvent>::new()));
            let pending_seat_events = seat_events.clone();

            event_loop
                .handle()
                .insert_source(
                    seat_notifier
                        .take()
                        .ok_or("Seat notifier already registered")?,
                    move |event, _, _| {
                        println!("Seat event received: {event:?}");
                        pending_seat_events.borrow_mut().push(event);
                    },
                )
                .map_err(|e| format!("Seat source failed: {e}"))?;

            let presented = Rc::new(Cell::new(false));
            let drm_presented = presented.clone();

            event_loop
                .handle()
                .insert_source(drm_notifier, move |event, _, _| match event {
                    DrmEvent::VBlank(output) if output == crtc => drm_presented.set(true),
                    DrmEvent::Error(e) => eprintln!("DRM event error: {e}"),
                    _ => {}
                })
                .map_err(|e| format!("DRM source failed: {e}"))?;

            let mut input_session = session.clone();

            event_loop
                .handle()
                .insert_source(
                    input_backend
                        .take()
                        .ok_or("Input backend already registered")?,
                    move |event, _, state| {
                        if let InputEvent::Keyboard { event } = event {
                            if let Some(keyboard) = state.seat.get_keyboard() {
                                keyboard.input::<(), _>(
                                    state,
                                    event.key_code(),
                                    event.state(),
                                    SERIAL_COUNTER.next_serial(),
                                    event.time_msec(),
                                    |_, _, key| {
                                        let sym = key.modified_sym().raw();
                                        let first = keysyms::KEY_XF86Switch_VT_1;
                                        let last = keysyms::KEY_XF86Switch_VT_12;

                                        if (first..=last).contains(&sym) {
                                            if event.state() == KeyState::Pressed {
                                                let vt = (sym - first + 1) as i32;
                                                if let Err(e) = input_session.change_vt(vt) {
                                                    eprintln!("TTY switch failed: {e}");
                                                }
                                            }
                                            return FilterResult::Intercept(());
                                        }
                                        FilterResult::Forward
                                    },
                                );
                            }
                        }
                    },
                )
                .map_err(|e| format!("Input source failed: {e}"))?;
            let start = Instant::now();
            let mut redraw = false;
            loop {
                // Read sysfs at most every 30 seconds.
                if Instant::now() >= next_battery_check {
                    next_battery_check = Instant::now() + Duration::from_secs(30);

                    if let Some(path) = &battery_path {
                        match BatteryStatus::read(path) {
                            Ok(status) => {
                                if battery_warning.update(&status) {
                                    match state.show_battery_warning(status.percent) {
                                        Ok(()) => redraw = true,
                                        Err(error) => {
                                            eprintln!("Battery notification failed: {error}");
                                        }
                                    }
                                }
                            }
                            Err(error) => {
                                eprintln!("Battery reading failed: {error}");
                            }
                        }
                    }
                }
                if let Some(status) = terminal
                    .try_wait()
                    .map_err(|e| format!("Terminal status failed: {e}"))?
                {
                    println!("Terminal exited: {status}");
                    break;
                }
                while let Some(stream) =
                    socket.accept().map_err(|e| format!("Accept failed: {e}"))?
                {
                    wayland
                        .handle()
                        .insert_client(stream, Arc::new(KeywimClientState::default()))
                        .map_err(|e| format!("Client registration failed: {e}"))?;
                }
                wayland
                    .dispatch_clients(&mut state)
                    .map_err(|e| format!("Client dispatch failed: {e}"))?;
                wayland
                    .flush_clients()
                    .map_err(|e| format!("Client flush failed: {e}"))?;

                event_loop
                    .dispatch(Some(Duration::from_millis(16)), &mut state)
                    .map_err(|e| format!("Dispatch failed: {e}"))?;

                for event in seat_events.borrow_mut().drain(..) {
                    match event {
                        SessionEvent::PauseSession => {
                            libinput.suspend();
                            device.pause();
                            presented.set(false);
                            println!("Seat paused");
                        }
                        SessionEvent::ActivateSession => {
                            device
                                .activate(true)
                                .map_err(|e| format!("DRM resume failed: {e}"))?;
                            libinput
                                .resume()
                                .map_err(|_| "Libinput resume failed".to_string())?;
                            buffered
                                .frame_submitted()
                                .map_err(|e| format!("Frame recovery failed: {e}"))?;
                            buffered.reset_buffers();
                            presented.set(false);
                            redraw = true;
                            println!("Seat resumed");
                        }
                    }
                }

                if !device.is_active() {
                    presented.set(false);
                    continue;
                }

                if presented.replace(false) {
                    buffered
                        .frame_submitted()
                        .map_err(|e| format!("Frame completion failed: {e}"))?;
                    redraw = true;
                }

                if redraw {
                    redraw = false;

                    for window in &state.windows {
                        window.with_pending_state(|pending| {
                            pending.size = Some((size.w, size.h).into());
                            pending.states.set(xdg_toplevel::State::Maximized);
                        });
                        window.send_pending_configure();
                    }
                    let (mut buffer, _) = buffered
                        .next_buffer()
                        .map_err(|e| format!("Next buffer failed: {e}"))?;
                    // Keep zone ordering centralized across output backends.
                    let elements = super::rendering::zone_elements(&mut renderer, &state)
                        .map_err(|e| format!("Zone elements failed: {e}"))?;

                    let sync = {
                        let mut target = renderer
                            .bind(&mut buffer)
                            .map_err(|e| format!("Bind failed: {e}"))?;
                        let mut frame = renderer
                            .render(&mut target, size, Transform::Normal)
                            .map_err(|e| format!("Render failed: {e}"))?;
                        frame
                            .clear(Color32F::new(0.0, 0.6, 0.7, 1.0), &[damage])
                            .map_err(|e| format!("Clear failed: {e}"))?;
                        draw_render_elements(&mut frame, 1.0, &elements, &[damage])
                            .map_err(|e| format!("Draw surfaces failed: {e}"))?;
                        frame.finish().map_err(|e| format!("Finish failed: {e}"))?
                    };
                    if let Err(e) = buffered.queue_buffer(Some(sync), None, ()) {
                        eprintln!("Queue failed before seat check: {e}");
                        event_loop
                            .dispatch(Some(Duration::from_millis(100)), &mut state)
                            .map_err(|e| format!("Diagnostic dispatch failed: {e}"))?;
                        if !session.is_active() {
                            redraw = true;
                            continue;
                        }
                        return Err(format!("Queue failed: {e}"));
                    }

                    for window in &state.windows {
                        send_frames_surface_tree(
                            window.wl_surface(),
                            start.elapsed().as_millis() as u32,
                        );
                    }
                }
            }

            // End successfully after the compositor session has finished.
            return Ok(());
        }
    }
    // All DRM devices were checked without finding a usable display.
    Err("No usable display output found".to_string())
}
