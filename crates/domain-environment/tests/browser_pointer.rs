use domain_environment::{
    BrowserFrameGeometry, BrowserPointerError, BrowserViewport, PaneBounds, PointerClientPosition,
    transform_browser_pointer,
};

fn geometry() -> BrowserFrameGeometry {
    BrowserFrameGeometry::new(
        "session-1",
        "tab-1",
        3,
        4,
        12,
        9,
        200,
        0.0,
        120.5,
        BrowserViewport::new(1280, 800, 2.0).unwrap(),
        PaneBounds::new(100.0, 50.0, 640.0, 400.0).unwrap(),
    )
    .unwrap()
}

#[test]
fn maps_displayed_frame_coordinates_to_chrome_css_viewport_coordinates() {
    let point = transform_browser_pointer(
        &geometry(),
        PointerClientPosition { x: 420.0, y: 250.0 },
        "session-1",
        "tab-1",
        3,
        4,
        12,
        9,
        220,
        100,
    )
    .unwrap();

    assert_eq!(point.x_css, 640.0);
    assert_eq!(point.y_css, 400.0);
}

#[test]
fn high_dpi_and_document_scroll_metadata_do_not_distort_viewport_coordinates() {
    let high_dpi = transform_browser_pointer(
        &geometry(),
        PointerClientPosition { x: 420.0, y: 250.0 },
        "session-1",
        "tab-1",
        3,
        4,
        12,
        9,
        220,
        100,
    )
    .unwrap();
    let different_dpr_and_scroll = BrowserFrameGeometry::new(
        "session-1",
        "tab-1",
        3,
        4,
        12,
        9,
        200,
        -18.25,
        2_000.75,
        BrowserViewport::new(1280, 800, 1.0).unwrap(),
        PaneBounds::new(100.0, 50.0, 640.0, 400.0).unwrap(),
    )
    .unwrap();
    let standard_dpi = transform_browser_pointer(
        &different_dpr_and_scroll,
        PointerClientPosition { x: 420.0, y: 250.0 },
        "session-1",
        "tab-1",
        3,
        4,
        12,
        9,
        220,
        100,
    )
    .unwrap();

    assert_eq!(high_dpi, standard_dpi);
    assert_eq!(
        different_dpr_and_scroll.scroll_offset_css(),
        (-18.25, 2_000.75)
    );
}

#[test]
fn rejects_pointer_events_from_a_stale_control_epoch() {
    assert_eq!(
        transform_browser_pointer(
            &geometry(),
            PointerClientPosition { x: 420.0, y: 250.0 },
            "session-1",
            "tab-1",
            3,
            3,
            12,
            9,
            220,
            100,
        ),
        Err(BrowserPointerError::StaleControlEpoch)
    );
}

#[test]
fn rejects_pointer_events_from_a_replaced_frame_or_resized_pane() {
    assert_eq!(
        transform_browser_pointer(
            &geometry(),
            PointerClientPosition { x: 420.0, y: 250.0 },
            "session-1",
            "tab-1",
            3,
            4,
            11,
            9,
            220,
            100,
        ),
        Err(BrowserPointerError::StaleFrame)
    );
    assert_eq!(
        transform_browser_pointer(
            &geometry(),
            PointerClientPosition { x: 420.0, y: 250.0 },
            "session-1",
            "tab-1",
            3,
            4,
            12,
            8,
            220,
            100,
        ),
        Err(BrowserPointerError::StaleGeometry)
    );
}

#[test]
fn rejects_pointer_positions_outside_the_browser_frame() {
    assert_eq!(
        transform_browser_pointer(
            &geometry(),
            PointerClientPosition { x: 99.0, y: 250.0 },
            "session-1",
            "tab-1",
            3,
            4,
            12,
            9,
            220,
            100,
        ),
        Err(BrowserPointerError::OutsideFrame)
    );
    assert_eq!(
        transform_browser_pointer(
            &geometry(),
            PointerClientPosition { x: 740.0, y: 250.0 },
            "session-1",
            "tab-1",
            3,
            4,
            12,
            9,
            220,
            100,
        ),
        Err(BrowserPointerError::OutsideFrame)
    );
}

#[test]
fn rejects_pointer_events_from_a_different_session_or_tab() {
    let geometry = geometry();
    assert_eq!(
        transform_browser_pointer(
            &geometry,
            PointerClientPosition { x: 420.0, y: 250.0 },
            "session-2",
            "tab-1",
            3,
            4,
            12,
            9,
            220,
            100,
        ),
        Err(BrowserPointerError::StaleSession)
    );
    assert_eq!(
        transform_browser_pointer(
            &geometry,
            PointerClientPosition { x: 420.0, y: 250.0 },
            "session-1",
            "tab-2",
            3,
            4,
            12,
            9,
            220,
            100,
        ),
        Err(BrowserPointerError::StaleTab)
    );
}

#[test]
fn rejects_frames_from_an_old_tab_generation() {
    assert_eq!(
        transform_browser_pointer(
            &geometry(),
            PointerClientPosition { x: 420.0, y: 250.0 },
            "session-1",
            "tab-1",
            2,
            4,
            12,
            9,
            220,
            100,
        ),
        Err(BrowserPointerError::StaleTabGeneration)
    );
}

#[test]
fn rejects_expired_or_future_browser_frames() {
    let geometry = geometry();
    assert_eq!(
        transform_browser_pointer(
            &geometry,
            PointerClientPosition { x: 420.0, y: 250.0 },
            "session-1",
            "tab-1",
            3,
            4,
            12,
            9,
            220,
            19,
        ),
        Err(BrowserPointerError::ExpiredFrame)
    );
    assert_eq!(
        transform_browser_pointer(
            &geometry,
            PointerClientPosition { x: 420.0, y: 250.0 },
            "session-1",
            "tab-1",
            3,
            4,
            12,
            9,
            199,
            100,
        ),
        Err(BrowserPointerError::FrameFromFuture)
    );
}

#[test]
fn rejects_non_finite_or_invalid_geometry() {
    assert_eq!(
        PaneBounds::new(100.0, 50.0, f64::NAN, 400.0),
        Err(BrowserPointerError::InvalidGeometry)
    );
    assert_eq!(
        BrowserViewport::new(1280, 800, f64::INFINITY),
        Err(BrowserPointerError::InvalidGeometry)
    );
    assert_eq!(
        transform_browser_pointer(
            &geometry(),
            PointerClientPosition {
                x: f64::NAN,
                y: 250.0,
            },
            "session-1",
            "tab-1",
            3,
            4,
            12,
            9,
            220,
            100,
        ),
        Err(BrowserPointerError::OutsideFrame)
    );
}
