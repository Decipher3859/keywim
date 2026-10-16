use super::{KeywimState, zones::NotificationZone};
use smithay::{
    backend::renderer::{
        RendererSuper,
        element::{
            Kind,
            memory::MemoryRenderBufferRenderElement,
            render_elements,
            surface::{WaylandSurfaceRenderElement, render_elements_from_surface_tree},
        },
        gles::GlesRenderer,
    },
    wayland::{compositor::with_states, shell::xdg::SurfaceCachedState},
};

render_elements! {pub ZoneRenderElement<=GlesRenderer>;
Notification=MemoryRenderBufferRenderElement<GlesRenderer>, Application=WaylandSurfaceRenderElement<GlesRenderer>,}

pub fn notification_element(
    renderer: &mut GlesRenderer,
    zone: &NotificationZone,
    scale: f64,
) -> Result<
    Option<MemoryRenderBufferRenderElement<GlesRenderer>>,
    <GlesRenderer as RendererSuper>::Error,
> {
    let Some(buffer) = &zone.buffer else {
        return Ok(None);
    };

    // Placement uses physical pixels; the zone stores logical coordinates.
    let location = zone.location.to_f64().to_physical(scale);
    MemoryRenderBufferRenderElement::from_buffer(
        renderer,
        location,
        buffer,
        None,
        None,
        None,
        Kind::Unspecified,
    )
    .map(Some)
}

pub fn zone_elements(
    renderer: &mut GlesRenderer,
    state: &KeywimState,
) -> Result<Vec<ZoneRenderElement>, <GlesRenderer as RendererSuper>::Error> {
    let mut elements = Vec::new();

    // Smithay expects front-to-back order, so notifications come first.
    if let Some(element) = notification_element(renderer, &state.notification_zone, 1.0)? {
        elements.push(ZoneRenderElement::Notification(element));
    }

    for window in &state.windows {
        // Align window geometry while preserving subsurface offsets.
        let offset = with_states(window.wl_surface(), |states| {
            states
                .cached_state
                .get::<SurfaceCachedState>()
                .current()
                .geometry
                .map(|g| (-g.loc.x, -g.loc.y))
                .unwrap_or((0, 0))
        });
        let window_elements = render_elements_from_surface_tree::<
            GlesRenderer,
            WaylandSurfaceRenderElement<GlesRenderer>,
        >(
            renderer,
            window.wl_surface(),
            offset,
            1.0,
            1.0,
            Kind::Unspecified,
        );
        elements.extend(
            window_elements
                .into_iter()
                .map(ZoneRenderElement::Application),
        );
    }
    Ok(elements)
}
