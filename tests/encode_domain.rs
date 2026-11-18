// ABOUTME: Checks the numeric domain that encode accepts for quantized attributes.
// ABOUTME: Covers refused ranges, boxes, bit counts, speeds, and the widest accepted inputs.
use tileforge_draco::{
    Attribute, AttributeType, DracoError, EncodeOptions, MeshView, Quantization, decode, encode,
};

const TRIANGLE: [f32; 9] = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
const GRID: EncodeOptions = EncodeOptions {
    position: Quantization::Grid { spacing: 1.0 },
    speed: 0,
};

/// A second attribute that lets Draco fit its own range.
fn fitted(kind: AttributeType, components: usize, bits: i32, data: &[f32]) -> Attribute<'_> {
    Attribute {
        kind,
        components,
        quantization_bits: bits,
        data,
        explicit: None,
    }
}

/// Encodes one triangle with unit positions and `second` as its other attribute.
fn encode_with(second: Attribute<'_>, opts: &EncodeOptions) -> Result<Vec<u8>, DracoError> {
    encode(
        MeshView {
            attributes: &[Attribute::positions(&TRIANGLE), second],
            indices: &[0, 1, 2],
            num_vertices: 3,
        },
        opts,
    )
    .map(|encoded| encoded.bytes)
}

/// Encodes one triangle that holds only `positions`.
fn encode_positions(positions: &[f32], opts: &EncodeOptions) -> Result<Vec<u8>, DracoError> {
    encode(
        MeshView {
            attributes: &[Attribute::positions(positions)],
            indices: &[0, 1, 2],
            num_vertices: 3,
        },
        opts,
    )
    .map(|encoded| encoded.bytes)
}

/// Runs one refused input and returns the word its message must hold.
fn refused_case(case: &str) -> (Result<Vec<u8>, DracoError>, &'static str) {
    let wide = [-3.0e38, 0.0, 3.0e38, 0.0, 0.0, 0.0];
    let zero = [0.0; 6];
    match case {
        // The fitted range is the largest component span. A span above the
        // largest finite float has no finite range.
        "fitted_span_above_float_max" => (
            encode_with(fitted(AttributeType::TexCoord, 2, 12, &wide), &GRID),
            "range",
        ),
        "fitted_color_span_above_float_max" => {
            let colors = [
                -3.0e38, 0.0, 0.0, 1.0, 3.0e38, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0,
            ];
            (
                encode_with(fitted(AttributeType::Color, 4, 8, &colors), &GRID),
                "range",
            )
        }
        "bits_position_span_above_float_max" => (
            encode_positions(
                &[-3.0e38, 0.0, 0.0, 3.0e38, 0.0, 0.0, 0.0, 1.0, 0.0],
                &EncodeOptions {
                    position: Quantization::Bits { bits: 14 },
                    speed: 0,
                },
            ),
            "range",
        ),
        // 4095 steps over a range of 1e-42 need a step count per unit that
        // binary32 cannot hold.
        "fitted_range_too_small" => {
            let narrow = [0.0, 0.0, 1.0e-42, 0.0, 0.0, 0.0];
            (
                encode_with(fitted(AttributeType::TexCoord, 2, 12, &narrow), &GRID),
                "range",
            )
        }
        "explicit_range_too_small" => {
            let origin = [0.0; 2];
            (
                encode_with(
                    Attribute {
                        explicit: Some((&origin, 1.0e-36)),
                        ..fitted(AttributeType::TexCoord, 2, 30, &zero)
                    },
                    &GRID,
                ),
                "range",
            )
        }
        "explicit_value_above_box" => {
            let uv = [0.0, 0.0, 1.5, 0.0, 0.0, 1.0];
            (
                encode_with(Attribute::unit_tex_coords(&uv, 12), &GRID),
                "outside",
            )
        }
        "explicit_value_below_box" => {
            let uv = [0.0, 0.0, 1.0, -0.25, 0.0, 1.0];
            (
                encode_with(Attribute::unit_tex_coords(&uv, 12), &GRID),
                "outside",
            )
        }
        "attribute_bits_above_30" => (
            encode_with(fitted(AttributeType::TexCoord, 2, 31, &zero), &GRID),
            "bit",
        ),
        "attribute_bits_negative" => (
            encode_with(fitted(AttributeType::TexCoord, 2, -1, &zero), &GRID),
            "bit",
        ),
        "position_bits_above_30" => (
            encode_positions(
                &TRIANGLE,
                &EncodeOptions {
                    position: Quantization::Bits { bits: 31 },
                    speed: 0,
                },
            ),
            "bit",
        ),
        "speed_above_10" => (
            encode_positions(&TRIANGLE, &EncodeOptions { speed: 11, ..GRID }),
            "speed",
        ),
        "speed_negative" => (
            encode_positions(&TRIANGLE, &EncodeOptions { speed: -1, ..GRID }),
            "speed",
        ),
        // Three times this vertex count wraps to 2 in 64-bit arithmetic, which
        // is the length of the slice below. The native count has 32 bits.
        "vertex_count_overflow" => (
            encode(
                MeshView {
                    attributes: &[Attribute::positions(&[0.0, 0.0])],
                    indices: &[0, 1, 2],
                    num_vertices: usize::MAX / 3 + 1,
                },
                // The grid check reads the positions and would refuse first.
                &EncodeOptions {
                    position: Quantization::Lossless,
                    speed: 0,
                },
            )
            .map(|encoded| encoded.bytes),
            "32 bits",
        ),
        _ => panic!("unknown case"),
    }
}

// Each refused input runs in its own process. A native abort must fail the
// parent assertion, rather than terminating the remainder of the test suite.
#[test]
fn inputs_outside_the_quantization_domain_return_argument_errors() {
    if let Ok(case) = std::env::var("TF_DRACO_DOMAIN_CASE") {
        let (result, word) = refused_case(&case);
        let error = result.unwrap_err();
        assert_eq!(error.code, 1, "{error}");
        assert!(error.message.contains(word), "{error}");
        return;
    }
    for case in [
        "fitted_span_above_float_max",
        "fitted_color_span_above_float_max",
        "bits_position_span_above_float_max",
        "fitted_range_too_small",
        "explicit_range_too_small",
        "explicit_value_above_box",
        "explicit_value_below_box",
        "attribute_bits_above_30",
        "attribute_bits_negative",
        "position_bits_above_30",
        "speed_above_10",
        "speed_negative",
        "vertex_count_overflow",
    ] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "inputs_outside_the_quantization_domain_return_argument_errors",
                "--nocapture",
            ])
            .env("TF_DRACO_DOMAIN_CASE", case)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{case}: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Decodes `bytes` and checks that every value of the attribute with unique id
/// 1 is within one quantization step of an input value.
fn assert_round_trip(bytes: &[u8], input: &[f32], step: f64) {
    let decoded = decode(bytes).unwrap();
    for value in decoded.read_f32(1).unwrap() {
        assert!(
            input
                .iter()
                .any(|&v| (v as f64 - value as f64).abs() <= step),
            "decoded {value} is more than one step from every input value"
        );
    }
}

#[test]
fn the_widest_fitted_ranges_encode_and_round_trip() {
    // A span of 3e38 is finite in binary32.
    let wide = [-1.0e38, 0.0, 2.0e38, 0.0, 0.0, 0.0];
    let bytes = encode_with(fitted(AttributeType::TexCoord, 2, 12, &wide), &GRID).unwrap();
    assert_round_trip(&bytes, &wide, 3.0e38 / 4095.0);

    // 4095 steps over 2e-35 is a finite step count per unit.
    let narrow = [0.0, 0.0, 2.0e-35, 0.0, 0.0, 0.0];
    let bytes = encode_with(fitted(AttributeType::TexCoord, 2, 12, &narrow), &GRID).unwrap();
    assert_round_trip(&bytes, &narrow, 2.0e-35 / 4095.0);

    // Equal values have a zero span, and the native code then uses a unit range.
    let equal = [0.25; 6];
    let bytes = encode_with(fitted(AttributeType::TexCoord, 2, 12, &equal), &GRID).unwrap();
    assert_round_trip(&bytes, &equal, 1.0 / 4095.0);
}

#[test]
fn values_on_the_edges_of_an_explicit_box_encode() {
    let uv = [0.0, -0.0, 1.0, 0.0, 0.0, 1.0];
    let bytes = encode_with(Attribute::unit_tex_coords(&uv, 12), &GRID).unwrap();
    assert_round_trip(&bytes, &uv, 1.0 / 4095.0);

    // A box away from zero. Its far edge is origin + range.
    let origin = [100.0, -4.0];
    let boxed = [100.0, -4.0, 108.0, -4.0, 100.0, 4.0];
    let bytes = encode_with(
        Attribute {
            explicit: Some((&origin, 8.0)),
            ..fitted(AttributeType::TexCoord, 2, 12, &boxed)
        },
        &GRID,
    )
    .unwrap();
    assert_round_trip(&bytes, &boxed, 8.0 / 4095.0);
}

#[test]
fn an_unquantized_attribute_ignores_its_explicit_box() {
    // With zero bits the native code stores the floats and never reads the box.
    let uv = [0.0, 0.0, 1.5, -0.25, 0.0, 1.0];
    let bytes = encode_with(Attribute::unit_tex_coords(&uv, 0), &GRID).unwrap();
    assert_round_trip(&bytes, &uv, 0.0);
}

#[test]
fn the_limits_of_the_bit_and_speed_ranges_encode() {
    let uv = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
    for bits in [1, 30] {
        encode_with(fitted(AttributeType::TexCoord, 2, bits, &uv), &GRID).unwrap();
        encode_with(Attribute::unit_tex_coords(&uv, bits), &GRID).unwrap();
        encode_positions(
            &TRIANGLE,
            &EncodeOptions {
                position: Quantization::Bits { bits },
                speed: 0,
            },
        )
        .unwrap();
    }
    for speed in [0, 10] {
        encode_positions(&TRIANGLE, &EncodeOptions { speed, ..GRID }).unwrap();
    }
}
