// ABOUTME: Checks that the decoded-mesh accessors refuse a mesh that is not usable.
// ABOUTME: Covers a face index that names no point of the decoded mesh.
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

/// The sequential triangle with its last face index changed to 3. The mesh
/// has points 0 to 2.
fn triangle_with_an_index_past_its_points() -> Vec<u8> {
    let mut bytes = sequential_triangle();
    assert_eq!(bytes[11..17], [1, 3, 1, 0, 1, 2], "the fixture moved");
    bytes[16] = 3;
    bytes
}

#[test]
fn the_native_decoder_refuses_a_face_index_that_names_no_point() {
    let error = decode(&triangle_with_an_index_past_its_points())
        .err()
        .expect("the decoder accepted a face index that names no point");
    assert_eq!(error.code, 3, "{error}");
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
    match decode(&bytes) {
        // The native decoder can refuse the bitstream before the accessor runs.
        Err(error) => assert_eq!(error.code, 3, "{error}"),
        Ok(mesh) => {
            let error = mesh.indices().unwrap_err();
            assert_eq!(error.code, 3, "{error}");
            assert!(error.message.contains("face index 3"), "{error}");
        }
    }
}
