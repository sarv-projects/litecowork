//! Fenced mapping from a displayed browser frame to Chrome CSS viewport coordinates.
//!
//! The caller still has to authorize and persist the browser input through the current
//! control lease and Effect path. This helper only rejects stale frame/geometry inputs
//! and translates coordinates; it never dispatches browser input.

use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserPointerError {
    InvalidGeometry,
    InvalidIdentity,
    StaleSession,
    StaleTab,
    StaleTabGeneration,
    StaleControlEpoch,
    StaleFrame,
    ExpiredFrame,
    FrameFromFuture,
    StaleGeometry,
    OutsideFrame,
}

impl fmt::Display for BrowserPointerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for BrowserPointerError {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrowserViewport {
    css_width: u32,
    css_height: u32,
    // Retained as frame metadata. Pointer coordinates enter in UI CSS pixels and CDP
    // dispatch also uses CSS pixels, so transforming them does not multiply by DPR.
    device_scale_factor: f64,
}

impl BrowserViewport {
    pub fn new(
        css_width: u32,
        css_height: u32,
        device_scale_factor: f64,
    ) -> Result<Self, BrowserPointerError> {
        if css_width == 0
            || css_height == 0
            || !device_scale_factor.is_finite()
            || !(0.5..=8.0).contains(&device_scale_factor)
        {
            return Err(BrowserPointerError::InvalidGeometry);
        }
        Ok(Self {
            css_width,
            css_height,
            device_scale_factor,
        })
    }

    pub const fn css_width(self) -> u32 {
        self.css_width
    }

    pub const fn css_height(self) -> u32 {
        self.css_height
    }

    pub const fn device_scale_factor(self) -> f64 {
        self.device_scale_factor
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaneBounds {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

impl PaneBounds {
    pub fn new(left: f64, top: f64, width: f64, height: f64) -> Result<Self, BrowserPointerError> {
        if ![left, top, width, height]
            .iter()
            .all(|value| value.is_finite())
            || left < 0.0
            || top < 0.0
            || width <= 0.0
            || height <= 0.0
            || !(left + width).is_finite()
            || !(top + height).is_finite()
        {
            return Err(BrowserPointerError::InvalidGeometry);
        }
        Ok(Self {
            left,
            top,
            width,
            height,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BrowserFrameGeometry {
    session_id: String,
    tab_id: String,
    tab_generation: u64,
    control_epoch: u64,
    frame_sequence: u64,
    // This is the screencast geometry/scroll transform revision, not a DOM revision.
    geometry_revision: u64,
    captured_at_ms: u64,
    // Scroll offset is captured with the frame for consumers that need document-space
    // provenance. CDP pointer dispatch uses viewport CSS coordinates, so it is not added
    // to the mapped point.
    scroll_x_css: f64,
    scroll_y_css: f64,
    viewport: BrowserViewport,
    pane: PaneBounds,
}

impl BrowserFrameGeometry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: impl Into<String>,
        tab_id: impl Into<String>,
        tab_generation: u64,
        control_epoch: u64,
        frame_sequence: u64,
        geometry_revision: u64,
        captured_at_ms: u64,
        scroll_x_css: f64,
        scroll_y_css: f64,
        viewport: BrowserViewport,
        pane: PaneBounds,
    ) -> Result<Self, BrowserPointerError> {
        let session_id = session_id.into();
        let tab_id = tab_id.into();
        if session_id.trim().is_empty() || tab_id.trim().is_empty() {
            return Err(BrowserPointerError::InvalidIdentity);
        }
        if tab_generation == 0
            || control_epoch == 0
            || frame_sequence == 0
            || geometry_revision == 0
            || !scroll_x_css.is_finite()
            || !scroll_y_css.is_finite()
        {
            return Err(BrowserPointerError::InvalidGeometry);
        }
        Ok(Self {
            session_id,
            tab_id,
            tab_generation,
            control_epoch,
            frame_sequence,
            geometry_revision,
            captured_at_ms,
            scroll_x_css,
            scroll_y_css,
            viewport,
            pane,
        })
    }

    pub const fn tab_generation(&self) -> u64 {
        self.tab_generation
    }

    pub const fn captured_at_ms(&self) -> u64 {
        self.captured_at_ms
    }

    pub const fn scroll_offset_css(&self) -> (f64, f64) {
        (self.scroll_x_css, self.scroll_y_css)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerClientPosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrowserViewportPosition {
    pub x_css: f64,
    pub y_css: f64,
}

/// Maps a pointer event only when it names the same session, tab generation, control
/// epoch, frame, and transform revision as the frame currently shown to the user. The
/// caller supplies the accepted frame-age bound from its provider policy; this pure helper
/// rejects future or over-age frames without choosing that policy itself.
#[allow(clippy::too_many_arguments)]
pub fn transform_browser_pointer(
    geometry: &BrowserFrameGeometry,
    position: PointerClientPosition,
    expected_session_id: &str,
    expected_tab_id: &str,
    expected_tab_generation: u64,
    expected_control_epoch: u64,
    expected_frame_sequence: u64,
    expected_geometry_revision: u64,
    now_ms: u64,
    max_frame_age_ms: u64,
) -> Result<BrowserViewportPosition, BrowserPointerError> {
    if geometry.session_id != expected_session_id {
        return Err(BrowserPointerError::StaleSession);
    }
    if geometry.tab_id != expected_tab_id {
        return Err(BrowserPointerError::StaleTab);
    }
    if geometry.tab_generation != expected_tab_generation {
        return Err(BrowserPointerError::StaleTabGeneration);
    }
    if geometry.control_epoch != expected_control_epoch {
        return Err(BrowserPointerError::StaleControlEpoch);
    }
    if geometry.frame_sequence != expected_frame_sequence {
        return Err(BrowserPointerError::StaleFrame);
    }
    if geometry.geometry_revision != expected_geometry_revision {
        return Err(BrowserPointerError::StaleGeometry);
    }
    if now_ms < geometry.captured_at_ms {
        return Err(BrowserPointerError::FrameFromFuture);
    }
    if now_ms - geometry.captured_at_ms > max_frame_age_ms {
        return Err(BrowserPointerError::ExpiredFrame);
    }
    if !position.x.is_finite() || !position.y.is_finite() {
        return Err(BrowserPointerError::OutsideFrame);
    }
    let x_in_pane = position.x - geometry.pane.left;
    let y_in_pane = position.y - geometry.pane.top;
    if x_in_pane < 0.0
        || y_in_pane < 0.0
        || x_in_pane >= geometry.pane.width
        || y_in_pane >= geometry.pane.height
    {
        return Err(BrowserPointerError::OutsideFrame);
    }
    Ok(BrowserViewportPosition {
        x_css: x_in_pane / geometry.pane.width * f64::from(geometry.viewport.css_width),
        y_css: y_in_pane / geometry.pane.height * f64::from(geometry.viewport.css_height),
    })
}
