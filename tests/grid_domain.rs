use tileforge_draco::*;

#[test]
fn subnormal_power_helpers_preserve_positive_targets() {
    for bit in 0..23 {
        let power = f32::from_bits(1 << bit);
        assert!(is_power_of_two(power));
        assert_eq!(power_of_two_at_most(power), power);
        let target = f32::from_bits((1 << (bit + 1)) - 1);
        assert_eq!(power_of_two_at_most(target), power);
        assert!(
            Quantization::grid(power).is_err(),
            "native grid needs a normal spacing"
        );
    }
}

#[test]
fn unsafe_grid_domains_return_argument_errors() {
    for (positions, spacing) in [
        (
            [
                1_000_000.0,
                0.0,
                0.0,
                1_000_001.0,
                0.0,
                0.0,
                1_000_000.0,
                1.0,
                0.0,
            ],
            1.0 / 4096.0,
        ),
        (
            [
                -1_000_000.0,
                0.0,
                0.0,
                -1_000_001.0,
                0.0,
                0.0,
                -1_000_000.0,
                1.0,
                0.0,
            ],
            1.0 / 4096.0,
        ),
        (
            [
                -1_000_000_000.0,
                0.0,
                0.0,
                1_000_000_000.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
            ],
            1.0,
        ),
        ([0.0, 0.0, 0.0, f32::MAX, 0.0, 0.0, 0.0, 1.0, 0.0], 1.0),
    ] {
        let atts = [Attribute::positions(&positions)];
        let error = encode(
            MeshView {
                attributes: &atts,
                indices: &[0, 1, 2],
                num_vertices: 3,
            },
            &EncodeOptions {
                position: Quantization::Grid { spacing },
                speed: 0,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("grid"));
    }
}
