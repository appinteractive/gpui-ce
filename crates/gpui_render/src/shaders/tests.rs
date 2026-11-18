use super::*;

#[test]
fn validates_subpixel_shader() {
    let generated = subpixel_sprite::WGSL_SOURCE.wgsl_source().unwrap();
    wgsl_rs::validate_wgsl_source(&format!("enable dual_source_blending;\n{generated}")).unwrap();
}

#[test]
fn build_generated_shaders_match_rust_sources() {
    assert_eq!(
        base::WGSL_SOURCE.wgsl_source().unwrap(),
        crate::artifacts::BASE_WGSL
    );
    let subpixel = subpixel_sprite::WGSL_SOURCE.wgsl_source().unwrap();
    assert_eq!(
        format!("enable dual_source_blending;\n{subpixel}"),
        crate::artifacts::SUBPIXEL_DUAL_SOURCE_WGSL
    );
}

#[test]
fn shader_interface_matches_generated_sources() {
    let standard_source = base::WGSL_SOURCE.wgsl_source().unwrap();
    let subpixel_source = subpixel_sprite::WGSL_SOURCE.wgsl_source().unwrap();
    for pipeline in interface::ALL {
        let source = if pipeline.label == interface::SUBPIXEL_SPRITES.label {
            &subpixel_source
        } else {
            &standard_source
        };
        assert!(
            source.contains(&format!("fn {}(", pipeline.vertex_entry)),
            "missing vertex entry point {}",
            pipeline.vertex_entry,
        );
        assert!(
            source.contains(&format!("fn {}(", pipeline.fragment_entry)),
            "missing fragment entry point {}",
            pipeline.fragment_entry,
        );
    }

    assert_eq!(common::GLOBALS.group, interface::GLOBAL_BIND_GROUP);
    assert_eq!(common::GLOBALS.binding, interface::GLOBAL_UNIFORMS_BINDING);
    assert_eq!(
        common::FONT_RASTERIZATION.group,
        interface::GLOBAL_BIND_GROUP
    );
    assert_eq!(
        common::FONT_RASTERIZATION.binding,
        interface::FONT_RASTERIZATION_BINDING
    );
    assert_eq!(quad::QUADS.group(), interface::DATA_BIND_GROUP);
    assert_eq!(quad::QUADS.binding(), interface::DATA_BUFFER_BINDING);
}

fn vertex_output_shape(module: &naga::Module, entry_name: &str) -> (usize, u32) {
    let entry = module
        .entry_points
        .iter()
        .find(|entry| entry.name == entry_name)
        .unwrap_or_else(|| panic!("missing shader entry point {entry_name}"));
    let result = entry
        .function
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("shader entry point {entry_name} has no output"));
    let naga::TypeInner::Struct { members, .. } = &module.types[result.ty].inner else {
        panic!("shader entry point {entry_name} must return a struct");
    };

    members
        .iter()
        .filter(|member| matches!(member.binding, Some(naga::Binding::Location { .. })))
        .fold((0, 0), |(locations, components), member| {
            let member_components = match module.types[member.ty].inner {
                naga::TypeInner::Scalar(_) => 1,
                naga::TypeInner::Vector { size, .. } => u32::from(size),
                ref ty => panic!("unsupported stage output type {ty:?} in {entry_name}"),
            };
            (locations + 1, components + member_components)
        })
}

#[test]
fn ordinary_vertex_interfaces_stay_within_compact_budgets() {
    let module = naga::front::wgsl::parse_str(&base::WGSL_SOURCE.wgsl_source().unwrap()).unwrap();
    for (entry, maximum_shape) in [
        ("vertex_quad", (8, 29)),
        ("vertex_shadow", (5, 17)),
        ("vertex_polychrome_sprite", (3, 7)),
        ("vertex_blur_composite", (2, 6)),
    ] {
        let actual_shape = vertex_output_shape(&module, entry);
        assert!(
            actual_shape.0 <= maximum_shape.0 && actual_shape.1 <= maximum_shape.1,
            "{entry} uses {actual_shape:?}, exceeding {maximum_shape:?} (locations, components)"
        );
    }
}

#[test]
fn scene_instance_storage_sizes_are_bounded() {
    for (name, actual, maximum) in [
        ("Quad", std::mem::size_of::<gpui::Quad>(), 248),
        ("Shadow", std::mem::size_of::<gpui::Shadow>(), 184),
        (
            "PolychromeSprite",
            std::mem::size_of::<gpui::PolychromeSprite>(),
            112,
        ),
    ] {
        assert!(
            actual <= maximum,
            "{name} is {actual} bytes, exceeding its {maximum}-byte storage budget"
        );
    }
}

#[test]
fn shader_buffer_layouts_match_host_layouts() {
    let modules = [
        crate::artifacts::BASE_WGSL,
        crate::artifacts::SUBPIXEL_DUAL_SOURCE_WGSL,
    ]
    .map(|source| naga::front::wgsl::parse_str(source).unwrap());

    for layouts in [gpui::SCENE_BUFFER_LAYOUTS, interface::RENDER_BUFFER_LAYOUTS] {
        for layout in layouts {
            let ty = modules
                .iter()
                .find_map(|module| {
                    module
                        .types
                        .iter()
                        .find_map(|(_, ty)| (ty.name.as_deref() == Some(layout.name)).then_some(ty))
                })
                .unwrap_or_else(|| panic!("missing shader ABI type {}", layout.name));
            let naga::TypeInner::Struct { members, span } = &ty.inner else {
                panic!("shader ABI type {} must be a struct", layout.name);
            };

            assert_eq!(
                *span as usize, layout.size,
                "shader ABI size: {}",
                layout.name
            );
            assert_eq!(
                members.len(),
                layout.fields.len(),
                "shader ABI fields: {}",
                layout.name
            );
            for (member, (name, offset)) in members.iter().zip(layout.fields) {
                assert_eq!(
                    member.name.as_deref(),
                    Some(*name),
                    "shader ABI field: {}",
                    layout.name
                );
                assert_eq!(
                    member.offset as usize, *offset,
                    "shader ABI offset: {}.{name}",
                    layout.name
                );
            }
        }
    }
}

#[test]
fn shader_discriminants_match_scene_types() {
    assert_eq!(common::ShaderBool::Disabled as u32, 0);
    assert_eq!(common::ShaderBool::Enabled as u32, 1);
    assert_eq!(
        common::BackgroundTag::Solid as u32,
        gpui::BackgroundTag::Solid as u32
    );
    assert_eq!(
        common::BackgroundTag::LinearGradient as u32,
        gpui::BackgroundTag::LinearGradient as u32
    );
    assert_eq!(
        common::BackgroundTag::PatternSlash as u32,
        gpui::BackgroundTag::PatternSlash as u32
    );
    assert_eq!(
        common::BackgroundTag::Checkerboard as u32,
        gpui::BackgroundTag::Checkerboard as u32
    );
    assert_eq!(
        common::ColorSpace::Srgb as u32,
        gpui::ColorSpace::Srgb as u32
    );
    assert_eq!(
        common::ColorSpace::Oklab as u32,
        gpui::ColorSpace::Oklab as u32
    );
    assert_eq!(
        common::BorderStyle::Solid as u32,
        gpui::BorderStyle::Solid as u32
    );
    assert_eq!(
        common::BorderStyle::Dashed as u32,
        gpui::BorderStyle::Dashed as u32
    );
}

#[test]
fn linear_gradients_preserve_native_dithering() {
    use common::*;
    use wgsl_rs::std::*;

    let color = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.5,
        a: 0.5,
    };
    let background = Background {
        tag: BackgroundTag::LinearGradient,
        color_space: ColorSpace::Srgb,
        solid: color,
        gradient_angle_or_pattern_height: 90.0,
        colors: [
            LinearColorStop {
                color,
                percentage: 0.0,
            },
            LinearColorStop {
                color,
                percentage: 1.0,
            },
        ],
        padding: 0,
    };
    let bounds = Bounds {
        origin: vec2f(0.0, 0.0),
        size: vec2f(100.0, 100.0),
    };
    let paint = Paint::new(background, bounds);
    let prepared = prepare_paint(paint);
    let first = paint_color(paint, vec2f(10.0, 10.0), prepared);
    let second = paint_color(paint, vec2f(11.0, 10.0), prepared);

    assert_ne!(first.w, second.w);
}

#[test]
fn ordered_dither_is_an_8x8_permutation() {
    use common::*;
    use wgsl_rs::std::*;

    let mut seen = [false; 64];
    for y in 0..8 {
        for x in 0..8 {
            let value = ordered_dither(vec2f(x as f32 + 0.5, y as f32 + 0.5));
            let index = (value * 64.0 - 0.5).round() as usize;
            assert!(
                (value - (index as f32 + 0.5) / 64.0).abs() < 1e-6,
                "{value} is not (i + 0.5) / 64"
            );
            assert!(!seen[index], "threshold {index} appears twice");
            seen[index] = true;
            assert_eq!(
                value,
                ordered_dither(vec2f(x as f32 + 8.5, y as f32 + 16.5)),
                "the pattern repeats every 8 pixels"
            );
        }
    }
    assert!(seen.iter().all(|&seen| seen));
    // The standard Bayer matrix starts 0 32 8 40 2 34 10 42.
    for (x, index) in [0, 32, 8, 40, 2, 34, 10, 42].into_iter().enumerate() {
        assert_eq!(
            ordered_dither(vec2f(x as f32 + 0.5, 0.5)),
            (index as f32 + 0.5) / 64.0
        );
    }
}

#[test]
fn coverage_dither_leaves_uncovered_fragments_alone() {
    use common::*;
    use wgsl_rs::std::*;

    for mode in [ShaderBool::Enabled, ShaderBool::Disabled] {
        let color = vec4f(0.1, 0.2, 0.3, 0.0);
        let out = coverage_dither_with(color, vec2f(3.5, 5.5), mode);
        assert_eq!((out.x, out.y, out.z, out.w), (0.1, 0.2, 0.3, 0.0));
    }
}

/// The destination value (0..1 per channel) after blending `source` over `destination` like the pipeline does.
fn blended(source: wgsl_rs::std::Vec4f, destination: f32, premultiplied: bool) -> f32 {
    let weight = if premultiplied { 1.0 } else { source.w };
    source.x * weight + destination * (1.0 - source.w)
}

#[test]
fn coverage_dither_moves_the_blend_by_less_than_one_step() {
    use common::*;
    use wgsl_rs::std::*;

    let destination = 27.0 / 255.0;
    let step = DITHER_STEP;
    for premultiplied in [true, false] {
        let mode = if premultiplied {
            ShaderBool::Enabled
        } else {
            ShaderBool::Disabled
        };
        for alpha in [0.002_f32, 0.05, 0.4] {
            let fade = (alpha * 255.0).min(1.0);
            // Black, so the premultiplied colour term is zero in both modes.
            let color = vec4f(0.0, 0.0, 0.0, alpha);
            let plain = blended(color, destination, premultiplied);
            let mut lighter = false;
            for y in 0..8 {
                for x in 0..8 {
                    let out =
                        coverage_dither_with(color, vec2f(x as f32 + 0.5, y as f32 + 0.5), mode);
                    assert_eq!(out.w, alpha, "alpha is never touched");
                    assert!(out.x >= 0.0, "rgb stays non-negative");
                    if premultiplied {
                        assert!(out.x <= alpha + 1e-7, "premultiplied rgb stays <= alpha");
                    } else {
                        assert!(out.x <= 1.0, "straight rgb stays <= 1");
                    }
                    let shift = blended(out, destination, premultiplied) - plain;
                    assert!(
                        shift >= -1e-7 && shift < step * fade + 1e-7,
                        "alpha {alpha} premultiplied {premultiplied}: shift {} steps",
                        shift / step
                    );
                    lighter |= shift > step * 0.5 * fade;
                }
            }
            assert!(lighter, "the upper thresholds are used (alpha {alpha})");
        }
    }
}

#[test]
fn coverage_dither_leaves_a_coloured_flat_blend_on_its_code() {
    use common::*;
    use wgsl_rs::std::*;

    // A coloured shadow whose blend lands exactly on a code must keep it at every pixel of the pattern
    // (the dither is centred, so it stays within half a step).
    let destination = 27.0 / 255.0;
    let color = vec4f(81.0 / 255.0, 81.0 / 255.0, 81.0 / 255.0, 0.5);
    let mut changed = 0;
    for y in 0..8 {
        for x in 0..8 {
            let out = coverage_dither_with(
                color,
                vec2f(x as f32 + 0.5, y as f32 + 0.5),
                ShaderBool::Disabled,
            );
            let before = (blended(color, destination, false) * 255.0).round();
            let after = (blended(out, destination, false) * 255.0).round();
            if before != after {
                changed += 1;
            }
        }
    }
    assert_eq!(changed, 0, "{changed} of 64 positions left their code");
}

#[test]
fn coverage_dither_removes_shadow_bands() {
    use common::*;
    use shadow::*;
    use wgsl_rs::std::*;

    let shadow = Shadow {
        order: 0,
        blur_radius: 32.0,
        bounds: Bounds {
            origin: vec2f(0.0, 0.0),
            size: vec2f(100.0, 200.0),
        },
        corner_radii: Corners {
            top_left: 0.0,
            top_right: 0.0,
            bottom_right: 0.0,
            bottom_left: 0.0,
        },
        content_mask: ContentMask {
            bounds: Bounds {
                origin: vec2f(-1000.0, -1000.0),
                size: vec2f(4000.0, 4000.0),
            },
            fade_out: Edges {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            },
        },
        color: Background {
            tag: BackgroundTag::Solid,
            color_space: ColorSpace::Srgb,
            solid: Hsla {
                h: 0.0,
                s: 0.0,
                l: 0.0,
                a: 0.5,
            },
            gradient_angle_or_pattern_height: 0.0,
            colors: [
                LinearColorStop {
                    color: Hsla {
                        h: 0.0,
                        s: 0.0,
                        l: 0.0,
                        a: 0.5,
                    },
                    percentage: 0.0,
                },
                LinearColorStop {
                    color: Hsla {
                        h: 0.0,
                        s: 0.0,
                        l: 0.0,
                        a: 0.5,
                    },
                    percentage: 1.0,
                },
            ],
            padding: 0,
        },
        element_bounds: Bounds {
            origin: vec2f(0.0, 0.0),
            size: vec2f(100.0, 200.0),
        },
        element_corner_radii: Corners {
            top_left: 0.0,
            top_right: 0.0,
            bottom_right: 0.0,
            bottom_left: 0.0,
        },
        inset: ShaderBool::Disabled,
        corner_smoothing: 0.0,
    };
    let destination = 27.0 / 255.0;
    let (first_row, first_x, last_x) = (96u32, 102u32, 198u32);
    // Longest run of equal 8x8 window sums along x, like the GPU test.
    let plateau = |dither: bool| {
        let value = |x: u32, y: u32| {
            let position = vec2f(x as f32 + 0.5, y as f32 + 0.5);
            let alpha = 0.5 * shadow_coverage(shadow, position);
            let mut color = vec4f(0.0, 0.0, 0.0, alpha);
            if dither {
                color = coverage_dither_with(color, position, ShaderBool::Disabled);
            }
            (blended(color, destination, false) * 255.0).round() as u32
        };
        let (mut longest, mut run, mut previous) = (0, 0, None);
        for x in first_x..last_x - 8 {
            let sum: u32 = (0..8)
                .flat_map(|dy| (0..8).map(move |dx| (dx, dy)))
                .map(|(dx, dy)| value(x + dx, first_row + dy))
                .sum();
            run = if previous == Some(sum) { run + 1 } else { 1 };
            previous = Some(sum);
            longest = Ord::max(longest, run);
        }
        longest
    };
    let (plain, dithered) = (plateau(false), plateau(true));
    assert!(plain >= 12, "the undithered shadow bands (plateau {plain})");
    assert!(
        dithered <= 8,
        "the dithered shadow still bands (plateau {dithered})"
    );
}
