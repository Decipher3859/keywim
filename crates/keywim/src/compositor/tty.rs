use smithay::{
    backend::{
        drm::DrmDeviceFd,
        session::{Session, libseat::LibSeatSession},
        udev::UdevBackend,
    },
    reexports::{
        drm::control::{Device as _, ModeTypeFlags, connector::State},
        rustix::fs::OFlags,
    },
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

            for encoder_handle in connector.encoders() {
                let encoder = drm
                    .get_encoder(*encoder_handle)
                    .map_err(|e| format!("Encoder query failed: {e}"))?;

                let crtcs = resources.filter_crtcs(encoder.possible_crtcs());
                println!("Possible CRTCs: {:?}", crtcs);
            }
        }
    }

    Ok(())
}
