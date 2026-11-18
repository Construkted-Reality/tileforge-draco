// ABOUTME: Checks that decode refuses a bitstream whose mesh is not usable.
// ABOUTME: Covers a face index that names no point of the mesh.
use tileforge_draco::{Attribute, EncodeOptions, MeshView, Quantization, decode, encode};

/// One triangle through the sequential codec, which stores its indices as
/// plain bytes.
fn sequential_triangle() -> Vec<u8> {
    let positions = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    encode(
        MeshView {
            attributes: &[Attribute::positions(&positions)],
            indices: &[0, 1, 2],
            num_vertices: 3,
        },
        &EncodeOptions {
            position: Quantization::Lossless,
            speed: 10,
        },
    )
    .unwrap()
    .bytes
}

#[test]
fn a_face_index_that_names_no_point_is_refused() {
    let mut bytes = sequential_triangle();
    // After the 11 header bytes: the face count, the point count, the marker
    // for plain indices, and one byte per index.
    assert_eq!(bytes[11..17], [1, 3, 1, 0, 1, 2], "the fixture moved");
    assert_eq!(decode(&bytes).unwrap().indices().unwrap(), [0, 1, 2]);

    // The mesh has points 0 to 2.
    bytes[16] = 3;
    let error = decode(&bytes)
        .err()
        .expect("the decoder accepted a face index that names no point");
    assert_eq!(error.code, 3, "{error}");
}
