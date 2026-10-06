// ABOUTME: Checks the grid spacing helpers and the native grid arithmetic domain.
// ABOUTME: Covers refused grids and the widest grids that round-trip exactly.
use tileforge_draco::*;

/// A triangle whose x coordinates span `lo` to `hi`.
fn span_triangle(lo: f32, hi: f32) -> [f32; 9] {
    [lo, 0.0, 0.0, hi, 0.0, 0.0, lo, 1.0, 0.0]
}

/// The widest inputs that the guard accepts. Each bound is exact in binary32.
/// tests/native_grid_domain.cc sends the same inputs to the native code.
#[test]
fn grids_at_the_edge_of_the_native_domain_round_trip_exactly() {
    for positions in [
        // -63 to 2^30 - 64 is exactly 2^30 grid values.
        span_triangle(-63.0, 1_073_741_760.0),
        // The lowest signed 32-bit index.
        span_triangle(-2_147_483_648.0, -2_147_482_624.0),
        // The highest binary32 index below 2^31.
        span_triangle(2_147_482_496.0, 2_147_483_520.0),
    ] {
        let atts = [Attribute::positions(&positions)];
        let encoded = encode(
            MeshView {
                attributes: &atts,
                indices: &[0, 1, 2],
                num_vertices: 3,
            },
            &EncodeOptions {
                position: Quantization::grid(1.0).unwrap(),
                speed: 0,
            },
        )
        .unwrap();
        let decoded = decode(&encoded.bytes).unwrap();
        let actual = decoded.read_f32(encoded.unique_ids[0]).unwrap();
        assert_eq!(actual.len(), positions.len());
        for point in actual.chunks_exact(3) {
            assert!(
                positions.chunks_exact(3).any(|input| input == point),
                "decoded {point:?} is not an input position"
            );
        }
    }
}

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
        // The first span past 2^30 values. Native binary32 counting rounds it
        // down to 2^30, so the guard is stricter than the native code here.
        (span_triangle(-64.0, 1_073_741_760.0), 1.0),
        // 2^30 + 129 values need 31 bits, and the native range computation
        // overflows a signed shift.
        (span_triangle(-128.0, 1_073_741_824.0), 1.0),
        // 2^31 does not fit in a signed 32-bit grid index.
        (span_triangle(2_147_482_624.0, 2_147_483_648.0), 1.0),
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
