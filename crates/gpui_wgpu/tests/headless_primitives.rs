//! Renders one of every primitive kind through the headless WGPU renderer and checks
//! that each one actually lands on the target. This is the cross-platform smoke test for
//! the shared render plan, instance transport, and generated shaders.
#![cfg(feature = "test-support")]

use gpui::{
    AtlasKey, AtlasTile, BackdropColorConversion, BackdropFilter, BackdropRowMask, BorderStyle,
    Bounds, ColorSpace, ContentMask, Corners, DevicePixels, Edges, FilterBoundary, Hsla,
    MonochromeSprite, PlatformHeadlessRenderer, Point, PolychromeSprite, Quad, RenderImageParams,
    RenderSvgParams, ScaledFilter, ScaledPixels, Scene, ShaderBool, Shadow, Size, Underline,
    checkerboard, linear_color_stop, linear_gradient, solid_background,
};
use gpui_ce_wgpu::WgpuHeadlessRenderer;
use smallvec::smallvec;
use std::borrow::Cow;

const TARGET: Size<DevicePixels> = Size {
    width: DevicePixels(310),
    height: DevicePixels(100),
};

#[test]
fn filled_path_resolves_without_invalidating_the_frame() {
    let mut renderer = WgpuHeadlessRenderer::new().expect("headless renderer");
    let mut scene = Scene::default();
    scene.insert_primitive(Quad {
        bounds: bounds(0.0, 0.0, 200.0, 100.0),
        content_mask: full_mask(),
        background: solid_background(gpui::black()),
        ..Default::default()
    });
    let mut builder = gpui::PathBuilder::fill();
    builder.move_to(gpui::point(gpui::px(20.0), gpui::px(20.0)));
    builder.line_to(gpui::point(gpui::px(80.0), gpui::px(20.0)));
    builder.line_to(gpui::point(gpui::px(50.0), gpui::px(80.0)));
    builder.close();
    let mut path = builder.build().expect("triangle").scale(1.0);
    path.content_mask = full_mask();
    path.color = solid_background(gpui::white());
    scene.insert_primitive(path);
    scene.finish();

    // The path uses an MSAA attachment when supported. Storing that transient
    // attachment invalidates the command buffer, including the background quad.
    let image = renderer
        .render_scene_to_image(&scene, TARGET)
        .expect("valid path frame");
    assert_eq!(image.get_pixel(50, 40).0, [255, 255, 255, 255]);
    assert_eq!(image.get_pixel(150, 40).0, [0, 0, 0, 255]);
}

fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds {
        origin: Point {
            x: ScaledPixels(x),
            y: ScaledPixels(y),
        },
        size: Size {
            width: ScaledPixels(w),
            height: ScaledPixels(h),
        },
    }
}

fn full_mask() -> ContentMask<ScaledPixels> {
    ContentMask {
        bounds: bounds(0.0, 0.0, 310.0, 100.0),
        ..Default::default()
    }
}

fn mask(width: f32, height: f32) -> ContentMask<ScaledPixels> {
    ContentMask {
        bounds: bounds(0.0, 0.0, width, height),
        ..Default::default()
    }
}

fn tile(renderer: &WgpuHeadlessRenderer, key: AtlasKey, bytes: Vec<u8>) -> AtlasTile {
    renderer
        .sprite_atlas()
        .get_or_insert_with(&key, &mut || {
            Ok(Some((
                Size {
                    width: DevicePixels(8),
                    height: DevicePixels(8),
                },
                Cow::Owned(bytes.clone()),
            )))
        })
        .expect("atlas insert must succeed")
        .expect("atlas insert must produce a tile")
}

#[test]
fn every_primitive_kind_renders() {
    let mut renderer = WgpuHeadlessRenderer::new().expect("headless renderer");
    let mono = tile(
        &renderer,
        AtlasKey::Svg(RenderSvgParams {
            path: "test-mono".into(),
            size: Size {
                width: DevicePixels(8),
                height: DevicePixels(8),
            },
        }),
        vec![255; 64],
    );
    let poly = tile(
        &renderer,
        AtlasKey::Image(RenderImageParams {
            image_id: gpui::ImageId(1),
            frame_index: 0,
        }),
        // BGRA red, opaque.
        (0..64).flat_map(|_| [0u8, 0, 255, 255]).collect(),
    );

    let mut scene = Scene::default();
    let green: Hsla = gpui::rgb_to_hsla(gpui::rgb(0x00ff00));
    let red: Hsla = gpui::rgb_to_hsla(gpui::rgb(0xff0000));
    let white: Hsla = gpui::rgb_to_hsla(gpui::rgb(0xffffff));
    let blue: Hsla = gpui::rgb_to_hsla(gpui::rgb(0x0000ff));

    // 1. Solid quad.
    let quad_bounds = bounds(10.0, 10.0, 30.0, 30.0);
    scene.insert_primitive(Quad {
        order: 0,
        bounds: quad_bounds,
        content_mask: full_mask(),
        background: solid_background(green),
        ..Default::default()
    });
    // 2. Bordered quad (white border, transparent fill).
    let bordered = bounds(50.0, 10.0, 30.0, 30.0);
    scene.insert_primitive(Quad {
        order: 0,
        bounds: bordered,
        content_mask: full_mask(),
        border_color: white.into(),
        border_widths: gpui::Edges::all(ScaledPixels(4.0)),
        ..Default::default()
    });
    // 3. Shadow (blue, no blur so it is a solid block).
    let shadow_bounds = bounds(90.0, 10.0, 30.0, 30.0);
    scene.insert_primitive(Shadow {
        order: 0,
        blur_radius: ScaledPixels(0.0),
        bounds: shadow_bounds,
        corner_radii: Default::default(),
        content_mask: full_mask(),
        color: blue.into(),
        element_bounds: shadow_bounds,
        element_corner_radii: Default::default(),
        inset: ShaderBool::Disabled,
        corner_smoothing: 0.0,
    });
    // 4. Underline (white, solid).
    let underline_bounds = bounds(130.0, 20.0, 30.0, 4.0);
    scene.insert_primitive(Underline {
        order: 0,
        padding: 0,
        bounds: underline_bounds,
        content_mask: full_mask(),
        color: white.into(),
        thickness: ScaledPixels(4.0),
        wavy: ShaderBool::Disabled,
    });
    // 5. Monochrome sprite (white coverage tile tinted green).
    let mono_bounds = bounds(10.0, 60.0, 30.0, 30.0);
    scene.insert_primitive(MonochromeSprite {
        order: 0,
        padding: 0,
        bounds: mono_bounds,
        content_mask: full_mask(),
        color: green.into(),
        tile: mono,
        transformation: Default::default(),
    });
    // 6. Polychrome sprite (red image).
    let poly_bounds = bounds(50.0, 60.0, 30.0, 30.0);
    scene.insert_primitive(PolychromeSprite {
        order: 0,
        grayscale: ShaderBool::Disabled,
        opacity: 1.0,
        corner_smoothing: 0.0,
        bounds: poly_bounds,
        content_mask: full_mask(),
        corner_radii: Default::default(),
        tile: poly,
    });
    // 7. A second quad batch. Overlapping the image pushes this quad above it in draw
    //    order, so it starts past the first quads in the frame's quad buffer: a backend
    //    that loses the batch base draws the wrong quads here.
    let late_quad = bounds(60.0, 70.0, 30.0, 30.0);
    scene.insert_primitive(Quad {
        order: 0,
        bounds: late_quad,
        content_mask: full_mask(),
        background: solid_background(blue),
        ..Default::default()
    });
    // 8–9. Gradient drop and inset shadows. The inset gradient stays in the element's paint
    // coordinate space while its offset hole is smaller.
    let gradient = linear_gradient(
        90.0,
        linear_color_stop(red, 0.0),
        linear_color_stop(blue, 1.0),
    )
    .color_space(ColorSpace::Srgb);
    let gradient_drop_bounds = bounds(210.0, 10.0, 40.0, 40.0);
    scene.insert_primitive(Shadow {
        order: 0,
        blur_radius: ScaledPixels(0.0),
        bounds: gradient_drop_bounds,
        corner_radii: Default::default(),
        content_mask: full_mask(),
        color: gradient,
        element_bounds: gradient_drop_bounds,
        element_corner_radii: Default::default(),
        inset: ShaderBool::Disabled,
        corner_smoothing: 0.0,
    });
    scene.insert_primitive(Shadow {
        order: 0,
        blur_radius: ScaledPixels(0.0),
        bounds: bounds(266.0, 16.0, 28.0, 28.0),
        corner_radii: Default::default(),
        content_mask: full_mask(),
        color: gradient,
        element_bounds: bounds(260.0, 10.0, 40.0, 40.0),
        element_corner_radii: Default::default(),
        inset: ShaderBool::Enabled,
        corner_smoothing: 0.0,
    });
    scene.finish();
    let quad_batches: Vec<_> = scene
        .render_commands()
        .iter()
        .filter_map(|command| match command {
            gpui::RenderCommand::Batch(gpui::PrimitiveBatch::Quads { range, .. }) => {
                Some(range.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        quad_batches,
        vec![0..2, 2..3],
        "the scene must split quads across batches"
    );

    let image = renderer
        .render_scene_to_image(&scene, TARGET)
        .expect("render must succeed");
    let px = |x: u32, y: u32| {
        let p = image.get_pixel(x, y).0;
        (p[0], p[1], p[2], p[3])
    };
    let mut failures = Vec::new();
    let mut check = |name: &str, x: u32, y: u32, expected: (u8, u8, u8)| {
        let (r, g, b, _) = px(x, y);
        let close = |a: u8, b: u8| (a as i32 - b as i32).abs() <= 8;
        if !(close(r, expected.0) && close(g, expected.1) && close(b, expected.2)) {
            failures.push(format!(
                "{name} at ({x},{y}): got ({r},{g},{b}) expected {expected:?}"
            ));
        }
    };
    check("solid quad", 25, 25, (0, 255, 0));
    check("bordered quad border", 52, 25, (255, 255, 255));
    check("bordered quad interior", 65, 25, (0, 0, 0));
    check("shadow", 105, 25, (0, 0, 255));
    check("underline", 145, 22, (255, 255, 255));
    check("monochrome sprite", 25, 75, (0, 255, 0));
    check("polychrome sprite", 55, 65, (255, 0, 0));
    check("second quad batch", 85, 85, (0, 0, 255));
    check("background", 190, 90, (0, 0, 0));
    check("gradient inset shadow hole", 280, 30, (0, 0, 0));
    let mut check_dominant = |name: &str, x: u32, y: u32, red_dominant: bool| {
        let (r, g, b, _) = px(x, y);
        let matches = if red_dominant {
            r > 180 && g < 80 && b < 80
        } else {
            b > 180 && r < 80 && g < 80
        };
        if !matches {
            failures.push(format!(
                "{name} at ({x},{y}): got ({r},{g},{b}), expected {} dominance",
                if red_dominant { "red" } else { "blue" }
            ));
        }
    };
    check_dominant("gradient drop shadow left edge", 212, 30, true);
    check_dominant("gradient drop shadow right edge", 247, 30, false);
    check_dominant("gradient inset shadow left edge", 262, 30, true);
    check_dominant("gradient inset shadow right edge", 297, 30, false);
    if !failures.is_empty() {
        let path = std::env::temp_dir().join("gpui_headless_primitives.png");
        image.save(&path).ok();
        panic!(
            "{}\n(image saved to {})",
            failures.join("\n"),
            path.display()
        );
    }
}

#[test]
fn smoothed_primitives_share_one_contour() {
    let mut renderer = WgpuHeadlessRenderer::new().expect("headless renderer");
    let image_tile = tile(
        &renderer,
        AtlasKey::Image(RenderImageParams {
            image_id: gpui::ImageId(2),
            frame_index: 0,
        }),
        (0..64).flat_map(|_| [0u8, 0, 255, 255]).collect(),
    );
    let target = Size {
        width: DevicePixels(360),
        height: DevicePixels(170),
    };
    let content_mask = mask(360.0, 170.0);
    let green: Hsla = gpui::rgb_to_hsla(gpui::rgb(0x19d36b));
    let white: Hsla = gpui::rgb_to_hsla(gpui::rgb(0xffffff));
    let blue: Hsla = gpui::rgb_to_hsla(gpui::rgb(0x287cff));
    let black: Hsla = gpui::rgb_to_hsla(gpui::rgb(0x000000));
    let radii = Corners::all(ScaledPixels(18.0));

    let mut scene = Scene::default();

    let fill_bounds = bounds(10.0, 10.0, 50.0, 50.0);
    scene.insert_primitive(Quad {
        bounds: fill_bounds,
        content_mask,
        background: solid_background(green),
        corner_radii: radii,
        corner_smoothing: 1.0,
        ..Default::default()
    });

    let border_bounds = bounds(72.0, 10.0, 50.0, 50.0);
    scene.insert_primitive(Quad {
        bounds: border_bounds,
        content_mask,
        border_color: white.into(),
        border_widths: Edges {
            top: ScaledPixels(2.0),
            right: ScaledPixels(5.0),
            bottom: ScaledPixels(8.0),
            left: ScaledPixels(3.0),
        },
        corner_radii: radii,
        corner_smoothing: 0.6,
        ..Default::default()
    });

    let dashed_bounds = bounds(134.0, 10.0, 58.0, 50.0);
    scene.insert_primitive(Quad {
        bounds: dashed_bounds,
        content_mask,
        border_style: BorderStyle::Dashed,
        border_color: white.into(),
        border_widths: Edges::all(ScaledPixels(3.0)),
        corner_radii: Corners {
            top_left: ScaledPixels(20.0),
            top_right: ScaledPixels(8.0),
            bottom_right: ScaledPixels(16.0),
            bottom_left: ScaledPixels(3.0),
        },
        corner_smoothing: 1.0,
        ..Default::default()
    });

    let image_bounds = bounds(204.0, 10.0, 50.0, 50.0);
    scene.insert_primitive(PolychromeSprite {
        order: 0,
        grayscale: ShaderBool::Disabled,
        opacity: 1.0,
        corner_smoothing: 1.0,
        bounds: image_bounds,
        content_mask,
        corner_radii: radii,
        tile: image_tile,
    });

    let drop_element = bounds(20.0, 96.0, 50.0, 44.0);
    scene.insert_primitive(Shadow {
        order: 0,
        blur_radius: ScaledPixels(5.0),
        bounds: bounds(25.0, 101.0, 50.0, 44.0),
        corner_radii: Corners::all(ScaledPixels(15.0)),
        content_mask,
        color: blue.into(),
        element_bounds: drop_element,
        element_corner_radii: Corners::all(ScaledPixels(15.0)),
        inset: ShaderBool::Disabled,
        corner_smoothing: 0.6,
    });
    scene.insert_primitive(Quad {
        bounds: drop_element,
        content_mask,
        background: solid_background(green),
        corner_radii: Corners::all(ScaledPixels(15.0)),
        corner_smoothing: 0.6,
        ..Default::default()
    });

    let inset_bounds = bounds(100.0, 94.0, 50.0, 48.0);
    scene.insert_primitive(Quad {
        bounds: inset_bounds,
        content_mask,
        background: solid_background(green),
        corner_radii: Corners::all(ScaledPixels(16.0)),
        corner_smoothing: 1.0,
        ..Default::default()
    });
    scene.insert_primitive(Shadow {
        order: 0,
        blur_radius: ScaledPixels(4.0),
        bounds: bounds(104.0, 98.0, 42.0, 40.0),
        corner_radii: Corners::all(ScaledPixels(12.0)),
        content_mask,
        color: black.into(),
        element_bounds: inset_bounds,
        element_corner_radii: Corners::all(ScaledPixels(16.0)),
        inset: ShaderBool::Enabled,
        corner_smoothing: 1.0,
    });

    let filter_bounds = bounds(190.0, 92.0, 70.0, 52.0);
    let backdrop = Quad {
        bounds: filter_bounds,
        content_mask,
        background: checkerboard(white, 2.0),
        ..Default::default()
    };
    scene.insert_primitive(backdrop);
    scene.insert_primitive(BackdropFilter {
        order: 0,
        bounds: filter_bounds,
        content_mask,
        corner_radii: Corners::all(ScaledPixels(18.0)),
        corner_smoothing: 1.0,
        filters: smallvec![ScaledFilter::Blur(ScaledPixels(5.0))],
        opacity: 1.0,
        row_mask: None,
        color_conversion: None,
    });

    scene.finish();
    let image = renderer
        .render_scene_to_image(&scene, target)
        .expect("render must succeed");
    let mut unfiltered_scene = Scene::default();
    unfiltered_scene.insert_primitive(backdrop);
    unfiltered_scene.finish();
    let unfiltered = renderer
        .render_scene_to_image(&unfiltered_scene, target)
        .expect("reference render must succeed");
    let pixel = |x: u32, y: u32| image.get_pixel(x, y).0;
    let reference_pixel = |x: u32, y: u32| unfiltered.get_pixel(x, y).0;
    let is_black = |x, y| pixel(x, y)[0..3].iter().all(|channel| *channel <= 12);
    let is_white = |x, y| pixel(x, y)[0..3].iter().all(|channel| *channel >= 235);

    for (name, left, center, channel, minimum) in
        [("fill", 10, 35, 1, 180), ("image", 204, 229, 0, 220)]
    {
        assert!(
            is_black(left, 10),
            "smoothed {name} must exclude its corner"
        );
        assert!(
            pixel(center, 10)[channel] > minimum,
            "{name} shoulder must reach the top edge",
        );
        assert!(pixel(center, 35)[channel] > minimum, "{name} center");
    }

    for (side, outer, inner) in [
        ("top", (96, 10), (96, 14)),
        ("right", (120, 35), (115, 35)),
        ("bottom", (96, 56), (96, 50)),
    ] {
        assert!(is_white(outer.0, outer.1), "{side} border");
        assert!(
            is_black(inner.0, inner.1),
            "{side} border interior boundary"
        );
    }

    let dashed_pixels = (8..62)
        .flat_map(|y| (132..194).map(move |x| (x, y)))
        .filter(|&(x, y)| is_white(x, y))
        .count();
    assert!(
        (150..420).contains(&dashed_pixels),
        "dashed contour coverage: {dashed_pixels}"
    );
    assert!(is_black(163, 35), "dashed border interior");

    assert!(
        pixel(73, 132)[2] > 35,
        "drop shadow must extend beyond the element"
    );
    assert!(
        pixel(125, 96)[1] < pixel(125, 118)[1],
        "inset shadow must darken the edge"
    );
    assert!(
        pixel(125, 118)[1] > 120,
        "inset shadow must preserve the center"
    );

    let blurred_midtones = (192..258)
        .flat_map(|x| (94..142).map(move |y| pixel(x, y)[0]))
        .filter(|channel| (30..225).contains(channel))
        .count();
    assert!(
        blurred_midtones > 800,
        "backdrop blur did not composite: {blurred_midtones}"
    );
    assert!(
        pixel(190, 92)
            .iter()
            .zip(reference_pixel(190, 92))
            .all(|(&actual, expected)| actual.abs_diff(expected) <= 1),
        "the smoothed mask must exclude the backdrop corner"
    );
    assert!(
        pixel(225, 92)[0].abs_diff(reference_pixel(225, 92)[0]) > 20,
        "the smoothed mask must include the top shoulder"
    );

    if std::env::var_os("GPUI_SAVE_HEADLESS_TESTS").is_some() {
        image
            .save(std::env::temp_dir().join("gpui_smoothed_primitives.png"))
            .expect("save diagnostic image");
    }
}

#[test]
fn content_mask_fades_quad_edges() {
    let mut renderer = WgpuHeadlessRenderer::new().expect("headless renderer");
    let mut scene = Scene::default();
    let red: Hsla = gpui::rgb_to_hsla(gpui::rgb(0xff0000));
    scene.insert_primitive(Quad {
        order: 0,
        bounds: bounds(0.0, 0.0, 310.0, 100.0),
        content_mask: ContentMask {
            bounds: bounds(0.0, 0.0, 100.0, 100.0),
            fade_out: Edges {
                top: ScaledPixels(10.0),
                right: ScaledPixels(0.0),
                bottom: ScaledPixels(0.0),
                left: ScaledPixels(20.0),
            },
        },
        background: solid_background(red),
        ..Default::default()
    });
    scene.finish();
    let image = renderer
        .render_scene_to_image(&scene, TARGET)
        .expect("render must succeed");
    let pixel = |x: u32, y: u32| image.get_pixel(x, y).0;
    let close = |x: u32, y: u32, red: u8| {
        let actual = pixel(x, y)[0];
        assert!(
            actual.abs_diff(red) <= 3,
            "at ({x},{y}) got {actual}, expected {red}"
        );
    };

    // Fully inside both fades.
    close(60, 50, 255);
    // The ramp is evaluated at pixel centers (x + 0.5), so 10.5px into the
    // 20px left fade is 52.5% coverage.
    close(10, 50, 134);
    // 2.5px into the 10px top fade is 25% coverage.
    close(60, 2, 64);
    // The unfaded right edge still clips hard at the mask boundary.
    assert_eq!(pixel(101, 50)[0], 0);
}

/// Renders a solid opaque quad of `color` across the whole target, then a backdrop filter over
/// the middle with the given chain, and returns the pixel at the centre of the filtered area.
fn backdrop_chain_pixel(color: u32, filters: &[ScaledFilter]) -> [u8; 4] {
    let mut renderer = WgpuHeadlessRenderer::new().expect("headless renderer");
    let mut scene = Scene::default();
    let fill: Hsla = gpui::rgb_to_hsla(gpui::rgb(color));
    scene.insert_primitive(Quad {
        bounds: bounds(0.0, 0.0, 310.0, 100.0),
        content_mask: full_mask(),
        background: solid_background(fill),
        ..Default::default()
    });
    scene.insert_primitive(BackdropFilter {
        order: 0,
        bounds: bounds(50.0, 20.0, 200.0, 60.0),
        content_mask: full_mask(),
        corner_radii: Corners::default(),
        corner_smoothing: 0.0,
        filters: filters.iter().copied().collect(),
        opacity: 1.0,
        row_mask: None,
        color_conversion: None,
    });
    scene.finish();
    let image = renderer
        .render_scene_to_image(&scene, TARGET)
        .expect("render must succeed");
    image.get_pixel(150, 50).0
}

#[test]
fn backdrop_saturate_zero_gives_the_luma_grey() {
    let pixel = backdrop_chain_pixel(
        0xc83c3c,
        &[
            ScaledFilter::Blur(ScaledPixels(2.0)),
            ScaledFilter::Saturate(0.0),
        ],
    );
    let luma = 0.2126 * 200.0 + 0.7152 * 60.0 + 0.0722 * 60.0;
    let channels = [pixel[0], pixel[1], pixel[2]];
    let spread = channels.iter().max().unwrap() - channels.iter().min().unwrap();
    assert!(spread <= 2, "channels must match: {pixel:?}");
    assert!(
        (f32::from(pixel[1]) - luma).abs() <= 3.0,
        "grey must be the luma {luma}: {pixel:?}"
    );
}

#[test]
fn backdrop_saturate_widens_the_channel_spread() {
    let blur = ScaledFilter::Blur(ScaledPixels(2.0));
    let base = backdrop_chain_pixel(0x965a5a, &[blur]);
    let boosted = backdrop_chain_pixel(0x965a5a, &[blur, ScaledFilter::Saturate(1.8)]);
    let base_spread = f32::from(base[0]) - f32::from(base[1]);
    let boosted_spread = f32::from(boosted[0]) - f32::from(boosted[1]);
    let ratio = boosted_spread / base_spread;
    assert!(
        (ratio - 1.8).abs() < 0.08,
        "spread ratio {ratio}: base {base:?}, boosted {boosted:?}"
    );
}

#[test]
fn backdrop_brightness_scales_each_channel() {
    let pixel = backdrop_chain_pixel(
        0xc83c3c,
        &[
            ScaledFilter::Blur(ScaledPixels(2.0)),
            ScaledFilter::Brightness(0.7),
        ],
    );
    for (channel, original) in [(0, 200.0_f32), (1, 60.0), (2, 60.0)] {
        let expected = original * 0.7;
        assert!(
            (f32::from(pixel[channel]) - expected).abs() <= 2.0,
            "channel {channel}: got {}, expected {expected}",
            pixel[channel]
        );
    }
}

#[test]
fn backdrop_color_stages_at_one_match_blur_alone() {
    let render = |filters: &[ScaledFilter]| {
        let mut renderer = WgpuHeadlessRenderer::new().expect("headless renderer");
        let mut scene = Scene::default();
        let warm: Hsla = gpui::rgb_to_hsla(gpui::rgb(0xc83c3c));
        let cool: Hsla = gpui::rgb_to_hsla(gpui::rgb(0x1e78dc));
        for stripe in 0..52 {
            scene.insert_primitive(Quad {
                bounds: bounds(stripe as f32 * 6.0, 0.0, 6.0, 100.0),
                content_mask: full_mask(),
                background: solid_background(if stripe % 2 == 0 { warm } else { cool }),
                ..Default::default()
            });
        }
        scene.insert_primitive(BackdropFilter {
            order: 0,
            bounds: bounds(50.0, 20.0, 200.0, 60.0),
            content_mask: full_mask(),
            corner_radii: Corners::all(ScaledPixels(10.0)),
            corner_smoothing: 0.0,
            filters: filters.iter().copied().collect(),
            opacity: 1.0,
            row_mask: None,
            color_conversion: None,
        });
        scene.finish();
        renderer
            .render_scene_to_image(&scene, TARGET)
            .expect("render must succeed")
    };
    let blur = ScaledFilter::Blur(ScaledPixels(3.0));
    let alone = render(&[blur]);
    let chained = render(&[
        blur,
        ScaledFilter::Saturate(1.0),
        ScaledFilter::Brightness(1.0),
    ]);
    assert_eq!(alone.as_raw(), chained.as_raw());
}

/// Renders a solid quad inside a content-filter group with the given chain and returns the pixel
/// at the centre of the group.
fn content_chain_pixel(color: u32, filters: &[ScaledFilter]) -> [u8; 4] {
    let mut renderer = WgpuHeadlessRenderer::new().expect("headless renderer");
    let mut scene = Scene::default();
    let start = FilterBoundary {
        order: 0,
        bounds: bounds(50.0, 20.0, 200.0, 60.0),
        content_mask: full_mask(),
        corner_radii: Corners::default(),
        corner_smoothing: 0.0,
        filters: filters.iter().copied().collect(),
        opacity: 1.0,
        is_start: true,
    };
    scene.insert_primitive(start.clone());
    scene.insert_primitive(Quad {
        bounds: bounds(50.0, 20.0, 200.0, 60.0),
        content_mask: full_mask(),
        background: solid_background(gpui::rgb_to_hsla(gpui::rgb(color))),
        ..Default::default()
    });
    scene.insert_primitive(FilterBoundary {
        is_start: false,
        ..start
    });
    scene.finish();
    let image = renderer
        .render_scene_to_image(&scene, TARGET)
        .expect("render must succeed");
    image.get_pixel(150, 50).0
}

#[test]
fn content_filter_saturate_works_without_blur() {
    let luma = 0.2126 * 200.0 + 0.7152 * 60.0 + 0.0722 * 60.0;
    let unfiltered = content_chain_pixel(0xc83c3c, &[ScaledFilter::Saturate(1.0)]);
    assert_eq!(
        [unfiltered[0], unfiltered[1], unfiltered[2]],
        [200, 60, 60],
        "a saturate(1) chain must leave the colour alone: {unfiltered:?}"
    );
    let pixel = content_chain_pixel(0xc83c3c, &[ScaledFilter::Saturate(0.0)]);
    let channels = [pixel[0], pixel[1], pixel[2]];
    let spread = channels.iter().max().unwrap() - channels.iter().min().unwrap();
    assert!(spread <= 2, "channels must match: {pixel:?}");
    assert!(
        (f32::from(pixel[1]) - luma).abs() <= 3.0,
        "grey must be the luma {luma}: {pixel:?}"
    );
    assert_eq!(pixel[3], 255, "the group stays opaque: {pixel:?}");
}

#[test]
fn content_filter_saturate_works_with_blur() {
    let luma = 0.2126 * 200.0 + 0.7152 * 60.0 + 0.0722 * 60.0;
    let pixel = content_chain_pixel(
        0xc83c3c,
        &[
            ScaledFilter::Blur(ScaledPixels(2.0)),
            ScaledFilter::Saturate(0.0),
        ],
    );
    let channels = [pixel[0], pixel[1], pixel[2]];
    let spread = channels.iter().max().unwrap() - channels.iter().min().unwrap();
    assert!(spread <= 2, "channels must match: {pixel:?}");
    assert!(
        (f32::from(pixel[1]) - luma).abs() <= 3.0,
        "grey must be the luma {luma}: {pixel:?}"
    );
    assert_eq!(pixel[3], 255, "the group stays opaque: {pixel:?}");
}

/// Renders 6 px vertical stripes under a backdrop blur whose content mask fades the bottom 40 px,
/// and returns the red-channel contrast between two neighbouring stripes at each sampled row.
fn faded_backdrop_contrast(rows: &[u32]) -> Vec<f32> {
    let mut renderer = WgpuHeadlessRenderer::new().expect("headless renderer");
    let mut scene = Scene::default();
    let warm: Hsla = gpui::rgb_to_hsla(gpui::rgb(0xc83c3c));
    let cool: Hsla = gpui::rgb_to_hsla(gpui::rgb(0x1e78dc));
    for stripe in 0..52 {
        scene.insert_primitive(Quad {
            bounds: bounds(stripe as f32 * 6.0, 0.0, 6.0, 100.0),
            content_mask: full_mask(),
            background: solid_background(if stripe % 2 == 0 { warm } else { cool }),
            ..Default::default()
        });
    }
    let filter_bounds = bounds(50.0, 20.0, 200.0, 60.0);
    scene.insert_primitive(BackdropFilter {
        order: 0,
        bounds: filter_bounds,
        content_mask: ContentMask {
            bounds: filter_bounds,
            fade_out: Edges {
                top: ScaledPixels(0.0),
                right: ScaledPixels(0.0),
                bottom: ScaledPixels(40.0),
                left: ScaledPixels(0.0),
            },
        },
        corner_radii: Corners::default(),
        corner_smoothing: 0.0,
        // The kernel runs on the half-resolution copy, so 4 px is about a 4 px deviation here.
        filters: smallvec![ScaledFilter::Blur(ScaledPixels(4.0))],
        opacity: 1.0,
        row_mask: None,
        color_conversion: None,
    });
    scene.finish();
    let image = renderer
        .render_scene_to_image(&scene, TARGET)
        .expect("render must succeed");
    // Stripes 14 and 15 are a warm and a cool stripe well inside the filter bounds.
    rows.iter()
        .map(|&y| f32::from(image.get_pixel(87, y).0[0]) - f32::from(image.get_pixel(93, y).0[0]))
        .collect()
}

#[test]
fn backdrop_composite_fades_with_the_content_mask() {
    // Rows every 5 px from the top of the filter (y = 20) to its last row (y = 79).
    let rows: Vec<u32> = (0..12).map(|step| 22 + step * 5).chain([79]).collect();
    let contrast = faded_backdrop_contrast(&rows);
    let at = |y: u32| contrast[rows.iter().position(|&row| row == y).unwrap()];
    let unblurred = 200.0 - 30.0;

    // The fade covers y = 40..80, so the top 15 px (y = 20..35) are fully blurred.
    for y in [22, 27, 32] {
        assert!(
            at(y) < 0.25 * unblurred,
            "row {y} should be blurred: {contrast:?}"
        );
    }
    // The bottom row is almost fully faded out, so the sharp stripes show through.
    assert!(
        at(79) > 0.95 * unblurred,
        "the bottom row should be unblurred: {contrast:?}"
    );
    // Between the two, contrast only rises (a small tolerance covers rounding).
    for pair in rows.windows(2).zip(contrast.windows(2)) {
        let (rows, values) = pair;
        assert!(
            values[1] >= values[0] - 2.0,
            "contrast must not fall between rows {} and {}: {contrast:?}",
            rows[0],
            rows[1]
        );
    }
    assert!(
        at(77) > at(42) + 0.5 * unblurred,
        "the fade must ramp over the bottom rows: {contrast:?}"
    );
}

/// Stripes under a backdrop blur whose composite follows a row mask. Returns the red-channel
/// contrast between a warm and a cool stripe for every row of the 100 px target.
fn row_masked_backdrop_contrast(row_mask: Option<BackdropRowMask>) -> Vec<f32> {
    let mut renderer = WgpuHeadlessRenderer::new().expect("headless renderer");
    let mut scene = Scene::default();
    let warm: Hsla = gpui::rgb_to_hsla(gpui::rgb(0xc83c3c));
    let cool: Hsla = gpui::rgb_to_hsla(gpui::rgb(0x1e78dc));
    for stripe in 0..52 {
        scene.insert_primitive(Quad {
            bounds: bounds(stripe as f32 * 6.0, 0.0, 6.0, 100.0),
            content_mask: full_mask(),
            background: solid_background(if stripe % 2 == 0 { warm } else { cool }),
            ..Default::default()
        });
    }
    let filter_bounds = bounds(50.0, 0.0, 200.0, 100.0);
    scene.insert_primitive(BackdropFilter {
        order: 0,
        bounds: filter_bounds,
        content_mask: ContentMask {
            bounds: filter_bounds,
            fade_out: Edges::default(),
        },
        corner_radii: Corners::default(),
        corner_smoothing: 0.0,
        filters: smallvec![ScaledFilter::Blur(ScaledPixels(8.0))],
        opacity: 1.0,
        row_mask,
        color_conversion: None,
    });
    scene.finish();
    let image = renderer
        .render_scene_to_image(&scene, TARGET)
        .expect("render must succeed");
    (0..100)
        .map(|y| f32::from(image.get_pixel(87, y).0[0]) - f32::from(image.get_pixel(93, y).0[0]))
        .collect()
}

#[test]
fn backdrop_composite_follows_the_row_mask() {
    let samples = [1.0, 1.0, 0.25, 0.75, 0.0];
    let mask = BackdropRowMask::new(ScaledPixels(20.0), ScaledPixels(10.0), &samples).unwrap();
    let masked = row_masked_backdrop_contrast(Some(mask));
    let sharp = 200.0 - 30.0;
    let blurred = row_masked_backdrop_contrast(None);
    // Coverage read back from the contrast: 1 where the composite is the blur alone, 0 where
    // the sharp stripes show.
    for y in [10usize, 25, 33, 38, 41, 45, 52, 57, 63, 80] {
        let expected = mask.alpha_at(y as f32 + 0.5);
        let read = (sharp - masked[y]) / (sharp - blurred[y]);
        assert!(
            (read - expected).abs() < 0.02,
            "row {y}: coverage {read:.3}, mask {expected:.3} (contrast {}, blurred {})",
            masked[y],
            blurred[y]
        );
    }
}

#[test]
fn row_mask_is_linear_between_samples_and_held_at_the_ends() {
    let mask =
        BackdropRowMask::new(ScaledPixels(20.0), ScaledPixels(10.0), &[0.5, 1.0, 0.0]).unwrap();
    assert_eq!(mask.alpha_at(0.0), 0.5);
    assert_eq!(mask.alpha_at(25.0), 0.75);
    assert_eq!(mask.alpha_at(35.0), 0.5);
    assert_eq!(mask.alpha_at(90.0), 0.0);
    assert!(BackdropRowMask::new(ScaledPixels(0.0), ScaledPixels(0.0), &[1.0]).is_none());
    assert!(BackdropRowMask::new(ScaledPixels(0.0), ScaledPixels(1.0), &[]).is_none());
    let clamped =
        BackdropRowMask::new(ScaledPixels(0.0), ScaledPixels(1.0), &[2.0, -1.0, f32::NAN]).unwrap();
    assert_eq!(&clamped.alphas[..3], &[1.0, 0.0, 1.0]);
}

/// Two-pixel row stripes, one per half-resolution texel row, under a near-identity backdrop blur
/// that composites with `row_mask`. Returns the red channel of column 100 for every row.
fn row_striped_backdrop_column(row_mask: Option<BackdropRowMask>) -> Vec<f32> {
    let mut renderer = WgpuHeadlessRenderer::new().expect("headless renderer");
    let mut scene = Scene::default();
    let warm: Hsla = gpui::rgb_to_hsla(gpui::rgb(0xc83c3c));
    let cool: Hsla = gpui::rgb_to_hsla(gpui::rgb(0x1e78dc));
    for texel in 0..50 {
        scene.insert_primitive(Quad {
            bounds: bounds(0.0, texel as f32 * 2.0, 300.0, 2.0),
            content_mask: full_mask(),
            background: solid_background(if texel % 2 == 0 { warm } else { cool }),
            ..Default::default()
        });
    }
    let filter_bounds = bounds(50.0, 0.0, 200.0, 100.0);
    scene.insert_primitive(BackdropFilter {
        order: 0,
        bounds: filter_bounds,
        content_mask: ContentMask {
            bounds: filter_bounds,
            fade_out: Edges::default(),
        },
        corner_radii: Corners::default(),
        corner_smoothing: 0.0,
        filters: smallvec![ScaledFilter::Blur(ScaledPixels(0.02))],
        opacity: 1.0,
        row_mask,
        color_conversion: None,
    });
    scene.finish();
    let image = renderer
        .render_scene_to_image(&scene, TARGET)
        .expect("render must succeed");
    (0..100)
        .map(|y| f32::from(image.get_pixel(100, y).0[0]))
        .collect()
}

#[test]
fn premultiplied_row_mask_weights_source_texel_rows() {
    // Mask samples at the texel row centres (device y 1, 3, 5, ...): coverage 1, 0.9, 0.1, 0.
    let samples = [1.0, 0.9, 0.1, 0.0];
    let plain = BackdropRowMask::new(ScaledPixels(1.0), ScaledPixels(2.0), &samples).unwrap();
    let premultiplied = plain.premultiplied();
    let output_masked = row_striped_backdrop_column(Some(plain));
    let source_masked = row_striped_backdrop_column(Some(premultiplied));
    // Source texel row i is warm (200) for even i and cool (30) for odd i, and so is the sharp
    // backdrop underneath, row y reading texel y / 2. Premultiplied: out = (1 - f) t0 C0 + f t1 C1
    // + sharp (1 - ((1 - f) t0 + f t1)) for f the fraction between texel row centres.
    let colour = |texel: usize| if texel % 2 == 0 { 200.0 } else { 30.0 };
    let coverage = |texel: usize| samples.get(texel).copied().unwrap_or(0.0);
    let mut largest_difference: f32 = 0.0;
    for y in 0..12usize {
        let position = (y as f32 + 0.5) / 2.0 - 0.5;
        let lower = position.floor().max(0.0) as usize;
        let upper = (position.floor() + 1.0).max(0.0) as usize;
        let fraction = position - position.floor();
        let fraction = if position < 0.0 { 0.0 } else { fraction };
        let (c0, c1) = (coverage(lower), coverage(upper));
        let source = (1.0 - fraction) * c0 * colour(lower) + fraction * c1 * colour(upper);
        let alpha = (1.0 - fraction) * c0 + fraction * c1;
        let expected = source + colour(y / 2) * (1.0 - alpha);
        assert!(
            (source_masked[y] - expected).abs() < 2.5,
            "row {y}: premultiplied {} against {expected:.1}",
            source_masked[y]
        );
        largest_difference = largest_difference.max((source_masked[y] - output_masked[y]).abs());
    }
    assert!(
        largest_difference > 8.0,
        "premultiplied and output-pixel masks must differ on row stripes, by {largest_difference}"
    );
}

/// Colour columns 10 px wide over the whole target, and in-place conversions of rows 10..50
/// (bounds at fractional y, so both conversions must round to the same pixel rows).
const CONVERSION_COLUMNS: [[u8; 3]; 11] = [
    [0, 0, 0],
    [64, 64, 64],
    [128, 128, 128],
    [192, 192, 192],
    [255, 255, 255],
    [255, 0, 0],
    [0, 255, 0],
    [0, 0, 255],
    [255, 255, 0],
    [255, 0, 255],
    [34, 39, 42],
];

fn converted_columns(conversions: &[BackdropColorConversion]) -> image::RgbaImage {
    let mut renderer = WgpuHeadlessRenderer::new().expect("headless renderer");
    let mut scene = Scene::default();
    for (index, [r, g, b]) in CONVERSION_COLUMNS.into_iter().enumerate() {
        let colour = gpui::rgb_to_hsla(gpui::rgb(u32::from_be_bytes([0, r, g, b])));
        scene.insert_primitive(Quad {
            bounds: bounds(index as f32 * 10.0, 0.0, 10.0, 100.0),
            content_mask: full_mask(),
            background: solid_background(colour),
            ..Default::default()
        });
    }
    let region = bounds(0.0, 10.4, 300.0, 39.8);
    for conversion in conversions {
        scene.insert_primitive(BackdropFilter {
            bounds: region,
            content_mask: full_mask(),
            color_conversion: Some(*conversion),
            ..Default::default()
        });
    }
    scene.finish();
    renderer
        .render_scene_to_image(&scene, TARGET)
        .expect("render must succeed")
}

#[test]
fn backdrop_conversion_reaches_display_p3_and_back() {
    let one_way = converted_columns(&[BackdropColorConversion::SrgbToDisplayP3]);
    let round_trip = converted_columns(&[
        BackdropColorConversion::SrgbToDisplayP3,
        BackdropColorConversion::DisplayP3ToSrgb,
    ]);
    let pixel = |image: &image::RgbaImage, column: usize, y: u32| {
        let p = image.get_pixel(column as u32 * 10 + 5, y).0;
        [p[0], p[1], p[2]]
    };
    for (column, colour) in CONVERSION_COLUMNS.into_iter().enumerate() {
        // Outside the region nothing changes, on both sides of the rounded edges.
        for y in [0, 9, 50, 99] {
            assert_eq!(
                pixel(&one_way, column, y),
                colour,
                "column {column} row {y} must be untouched"
            );
        }
        // The target keeps 8 bits between the two conversions: greys (and the near-grey window
        // colour) come back exactly, saturated colours within a few levels (pure green reads
        // 3, 255, 0, as the analytic model of the two roundings predicts).
        let grey = colour[0] == colour[1] && colour[1] == colour[2];
        for y in [10, 30, 49] {
            let back = pixel(&round_trip, column, y);
            if grey || column == 10 {
                assert_eq!(back, colour, "column {column} row {y}: round trip");
            } else {
                let worst = back
                    .iter()
                    .zip(colour)
                    .map(|(a, b)| a.abs_diff(b))
                    .max()
                    .unwrap();
                assert!(
                    worst <= 3,
                    "column {column} row {y}: round trip reads {back:?}"
                );
            }
        }
        if grey {
            assert_eq!(
                pixel(&one_way, column, 30),
                colour,
                "greys convert to themselves"
            );
        }
    }
    // sRGB red in Display P3 (FIT.md: the display profile reads 234, 51, 35).
    let red = pixel(&one_way, 5, 30);
    for (got, expected) in red.iter().zip([234u8, 51, 35]) {
        assert!(
            got.abs_diff(expected) <= 1,
            "sRGB red in Display P3 reads {red:?}"
        );
    }
    assert_eq!(
        pixel(&one_way, 10, 30),
        [35, 39, 42],
        "the dark window colour in Display P3"
    );
}

/// Longest run of consecutive x whose 8x8 window sum of R+G+B is equal. Banded ramps give long runs;
/// a dithered ramp changes the sum nearly every pixel.
fn longest_plateau(
    image: &image::RgbaImage,
    rows: std::ops::Range<u32>,
    xs: std::ops::Range<u32>,
) -> u32 {
    let mut longest = 0;
    let mut run = 0;
    let mut previous = None;
    for x in xs.start..xs.end - 8 {
        let mut sum = 0u32;
        for y in rows.start..rows.start + 8 {
            for dx in 0..8 {
                let [r, g, b, _] = image.get_pixel(x + dx, y).0;
                sum += u32::from(r) + u32::from(g) + u32::from(b);
            }
        }
        run = if previous == Some(sum) { run + 1 } else { 1 };
        previous = Some(sum);
        longest = longest.max(run);
    }
    longest
}

fn blurred_shadow_image(renderer: &mut WgpuHeadlessRenderer) -> image::RgbaImage {
    let target = Size {
        width: DevicePixels(420),
        height: DevicePixels(300),
    };
    let mut scene = Scene::default();
    scene.insert_primitive(Quad {
        order: 0,
        bounds: bounds(0.0, 0.0, 420.0, 300.0),
        content_mask: mask(420.0, 300.0),
        background: solid_background(gpui::rgb_to_hsla(gpui::rgb(0x1b1c1f))),
        ..Default::default()
    });
    let shadow_bounds = bounds(40.0, 40.0, 120.0, 220.0);
    scene.insert_primitive(Shadow {
        order: 1,
        blur_radius: ScaledPixels(32.0),
        bounds: shadow_bounds,
        corner_radii: Default::default(),
        content_mask: mask(420.0, 300.0),
        color: gpui::hsla(0.0, 0.0, 0.0, 0.5).into(),
        element_bounds: shadow_bounds,
        element_corner_radii: Default::default(),
        inset: ShaderBool::Disabled,
        corner_smoothing: 0.0,
    });
    scene.finish();
    renderer
        .render_scene_to_image(&scene, target)
        .expect("shadow frame")
}

/// A dark blurred shadow ramps over ~27 codes; with 8-bit output that bands unless the shader dithers it.
/// The headless target is opaque, which is the straight-alpha path of the macOS window.
#[test]
fn blurred_shadow_has_no_bands() {
    let mut renderer = WgpuHeadlessRenderer::new().expect("headless renderer");
    let image = blurred_shadow_image(&mut renderer);
    if let Ok(dir) = std::env::var("GPUI_DITHER_PPM_DIR") {
        // The image crate here has no PNG encoder; write a binary PPM (convert it with any image tool).
        let mut ppm = format!("P6\n{} {}\n255\n", image.width(), image.height()).into_bytes();
        ppm.extend(
            image
                .pixels()
                .flat_map(|pixel| [pixel.0[0], pixel.0[1], pixel.0[2]]),
        );
        std::fs::write(std::path::Path::new(&dir).join("gpui-shadow.ppm"), ppm).expect("save ppm");
    }

    let (rows, xs) = (146..154, 164..256);
    let mean = |range: std::ops::Range<u32>| -> f32 {
        let mut sum = 0.0;
        let mut count = 0.0;
        for x in range {
            for y in rows.clone() {
                sum += f32::from(image.get_pixel(x, y).0[0]);
                count += 1.0;
            }
        }
        sum / count
    };
    // The shadow darkens toward the box and the strip starts at the panel background.
    assert!(
        mean(164..172) < mean(248..256),
        "the ramp darkens toward the box"
    );
    let plateau = longest_plateau(&image, rows, xs);
    eprintln!("shadow plateau: {plateau}");
    assert!(plateau <= 8, "shadow bands: longest equal run {plateau} px");

    let again = blurred_shadow_image(&mut renderer);
    assert_eq!(
        image.as_raw(),
        again.as_raw(),
        "the dither is deterministic"
    );
}
