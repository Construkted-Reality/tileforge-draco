// ABOUTME: Checks lossless position encoding against oriented triangle attribute values.
// ABOUTME: Exercises native reordering, shared vertices, wide coordinates, and invalid inputs.
use tileforge_draco::{
    Attribute, AttributeType, EncodeOptions, MeshView, Quantization, decode, encode,
};

// Cyclic rotation preserves winding. Sorting retains repeated triangles.
fn triangles(positions: &[f32], uv: &[f32], indices: &[u32]) -> Vec<[[u32; 5]; 3]> {
    let mut faces: Vec<_> = indices
        .chunks_exact(3)
        .map(|face| {
            let vertices: [[u32; 5]; 3] = std::array::from_fn(|i| {
                let v = face[i] as usize;
                std::array::from_fn(|c| {
                    let value = if c < 3 {
                        positions[v * 3 + c]
                    } else {
                        uv[v * 2 + c - 3]
                    };
                    if value == 0.0 { 0 } else { value.to_bits() }
                })
            });
            [
                vertices,
                [vertices[1], vertices[2], vertices[0]],
                [vertices[2], vertices[0], vertices[1]],
            ]
            .into_iter()
            .min()
            .unwrap()
        })
        .collect();
    faces.sort();
    faces
}

#[test]
fn lossless_preserves_oriented_triangles_and_unquantized_uv() {
    for positions in [
        vec![
            0.125, -0.0, 0.1, 0.375, 0.0, 0.2, 0.125, 0.375, -0.3, 0.375, 0.375, 0.4,
        ],
        vec![
            -1.0e20, 0.0, 0.1, 1.0e20, 0.0, 0.2, -1.0e20, 1.0e20, -0.3, 1.0e20, 1.0e20, 0.4,
        ],
    ] {
        let uv = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
        let indices = [0, 1, 2, 1, 3, 2, 0, 1, 2, 2, 1, 0];
        for speed in [0, 10] {
            let encoded = encode(
                MeshView {
                    attributes: &[
                        Attribute::positions(&positions),
                        Attribute {
                            kind: AttributeType::TexCoord,
                            components: 2,
                            quantization_bits: 0,
                            data: &uv,
                            explicit: None,
                        },
                    ],
                    indices: &indices,
                    num_vertices: positions.len() / 3,
                },
                &EncodeOptions {
                    position: Quantization::Lossless,
                    speed,
                },
            )
            .unwrap();
            let decoded = decode(&encoded.bytes).unwrap();
            assert_eq!(decoded.num_faces(), 4);
            assert_eq!(
                triangles(&positions, &uv, &indices),
                triangles(
                    &decoded.read_f32(encoded.unique_ids[0]).unwrap(),
                    &decoded.read_f32(encoded.unique_ids[1]).unwrap(),
                    &decoded.indices().unwrap(),
                )
            );
        }
    }
}

#[test]
fn lossless_keeps_other_attribute_quantization() {
    let positions = [0.125, 0.0, 0.1, 0.375, 0.0, 0.2, 0.125, 0.375, -0.3];
    let uv = [0.1234, 0.2345, 0.3456, 0.4567, 0.5678, 0.6789];
    let encoded = encode(
        MeshView {
            attributes: &[
                Attribute::positions(&positions),
                Attribute::unit_tex_coords(&uv, 10),
            ],
            indices: &[0, 1, 2],
            num_vertices: 3,
        },
        &EncodeOptions {
            position: Quantization::Lossless,
            speed: 0,
        },
    )
    .unwrap();
    let decoded = decode(&encoded.bytes).unwrap();
    let actual = decoded.read_f32(encoded.unique_ids[1]).unwrap();
    assert!(actual.iter().any(|v| !uv.contains(v)));
    for pair in actual.chunks_exact(2) {
        assert!(
            uv.chunks_exact(2)
                .any(|source| (pair[0] - source[0]).abs() <= 1.0 / 1023.0
                    && (pair[1] - source[1]).abs() <= 1.0 / 1023.0)
        );
    }
}

#[test]
fn lossless_rejects_invalid_inputs_and_bits_zero_stays_invalid() {
    let valid = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut positions = valid;
        positions[0] = value;
        let error = encode(
            MeshView {
                attributes: &[Attribute::positions(&positions)],
                indices: &[0, 1, 2],
                num_vertices: 3,
            },
            &EncodeOptions {
                position: Quantization::Lossless,
                speed: 0,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("finite"));
    }
    let error = encode(
        MeshView {
            attributes: &[Attribute::positions(&valid)],
            indices: &[0, 1, 3],
            num_vertices: 3,
        },
        &EncodeOptions {
            position: Quantization::Lossless,
            speed: 0,
        },
    )
    .unwrap_err();
    assert_eq!(error.code, 1);
    assert!(error.message.contains("index"));
    let error = encode(
        MeshView {
            attributes: &[Attribute::positions(&valid)],
            indices: &[0, 1, 2],
            num_vertices: 3,
        },
        &EncodeOptions {
            position: Quantization::Bits { bits: 0 },
            speed: 0,
        },
    )
    .unwrap_err();
    assert_eq!(error.code, 1);
    assert!(error.message.contains("positive"));
}
