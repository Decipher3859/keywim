use super::{KeywimClientState, KeywimState, helpers::send_frames_surface_tree};
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
        renderer::{
            Bind, Color32F, Frame, Renderer,
            element::{
                Kind,
                surface::{WaylandSurfaceRenderElement, render_elements_from_surface_tree},
            },
            gles::GlesRenderer,
            utils::draw_render_elements,
        },
        session::{Session, libseat::LibSeatSession},
        udev::UdevBackend,
    },
    reexports::{
        drm::control::{Device as _, ModeTypeFlags, connector::State},
        rustix::fs::OFlags,
        wayland_server::{Display, ListeningSocket},
    },
    utils::{Rectangle, Transform},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub fn run_tty() -> Result<(), String> {
    let (mut session, _notifier) =
        LibSeatSession::new().map_err(|e| format!("Seat initialization failed: {e}"))?;
    println!("Seat opened: {}", session.seat());
    let udev =
        UdevBackend::new(session.seat()).map_err(|e| format!("Udev initialization failed: {e}"))?;

    for (_, path) in udev.device_list() {
        println!("DRM device: {}", path.display());
        let fd = session
            .open(path, OFlags::RDWR | OFlags::CLOEXEC)
            .map_err(|e| format!("DRM open failed: {e}"))?;
        println!("DRM opened: {}", path.display());

        let drm = DrmDeviceFd::new(fd.into());
        let resources = drm
            .resource_handles()
            .map_err(|e| format!("DRM resources failed: {e}"))?;

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

            if connector.state() != State::Connected {
                continue;
            }

            let mode = connector
                .modes()
                .iter()
                .find(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
                .or_else(|| connector.modes().first())
                .copied()
                .ok_or_else(|| "Connected display has no modes".to_string())?;

            println!("Selected mode: {:?}", mode);

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

            let crtc = selected_crtc.ok_or_else(|| "No compatible CRTC found".to_string())?;

            println!(
                "Selected output: {:?}-{}, CRTC: {:?}, mode: {:?}",
                connector.interface(),
                connector.interface_id(),
                crtc,
                mode,
            );

            let (mut device, drm_notifier) = DrmDevice::new(drm.clone(), false)
                .map_err(|e| format!("DRM initialization failed: {e}"))?;

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
            let mut event_loop =
                EventLoop::<bool>::try_new().map_err(|e| format!("Eventloop failed: {e}"))?;

            let mut wayland = Display::<KeywimState>::new()
                .map_err(|e| format!("Wayland dislpay failed: {e}"))?;
            let mut state = KeywimState::new(&wayland.handle());
            let socket = ListeningSocket::bind_auto("wayland", 1..32)
                .map_err(|e| format!("Wayland socket failed: {e}"))?;
            let socket_name = socket.socket_name().unwrap().to_string_lossy();
            let mut terminal = crate::autostart::launch_alacritty(&socket_name)
                .map_err(|e| format!("Alacritty failed: {e}"))?;

            event_loop
                .handle()
                .insert_source(drm_notifier, move |event, _, presented| match event {
                    DrmEvent::VBlank(output) if output == crtc => *presented = true,
                    DrmEvent::Error(e) => eprintln!("DRM event error: {e}"),
                    _ => {}
                })
                .map_err(|e| format!("DRM source failed: {e}"))?;
            let start = Instant::now();
            let mut presented = false;
            while start.elapsed() < Duration::from_secs(10) {
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
                    .dispatch(Some(Duration::from_millis(16)), &mut presented)
                    .map_err(|e| format!("Dispatch failed: {e}"))?;

                if presented {
                    buffered
                        .frame_submitted()
                        .map_err(|e| format!("Frame completion failed: {e}"))?;
                    presented = false;

                    let (mut buffer, _) = buffered
                        .next_buffer()
                        .map_err(|e| format!("Next buffer failed: {e}"))?;
                    let elements = state
                        .windows
                        .iter()
                        .flat_map(|window| {
                            render_elements_from_surface_tree(
                                &mut renderer,
                                window.wl_surface(),
                                (0, 0),
                                1.0,
                                1.0,
                                Kind::Unspecified,
                            )
                        })
                        .collect::<Vec<WaylandSurfaceRenderElement<GlesRenderer>>>();

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
                    buffered
                        .queue_buffer(Some(sync), None, ())
                        .map_err(|e| format!("Queue failed: {e}"))?;

                    for window in &state.windows {
                        send_frames_surface_tree(
                            window.wl_surface(),
                            start.elapsed().as_millis() as u32,
                        );
                    }
                }
            }

            let _ = terminal.kill();
            let _ = terminal.wait();

            break;
        }
    }

    Ok(())
}
