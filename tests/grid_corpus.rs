//! ABOUTME: Remote corpus gate for the shared snapping and native codec contract.
//! ABOUTME: Checks grid positions and unit-square texture coordinates through Draco.
use std::collections::{HashMap, HashSet};
use tileforge_draco::{
    Attribute, EncodeOptions, MeshView, Quantization, decode, encode, snap_positions,
};

/// Texture-coordinate depths the callers use. tileforge-optimize uses 10 bits.
/// tileforge-mesh derives 11 to 16 bits from the atlas size.
const UV_BITS: std::ops::RangeInclusive<i32> = 10..=16;

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn key(position: &[f32]) -> [u32; 3] {
    std::array::from_fn(|i| {
        if position[i] == 0.0 {
            0
        } else {
            position[i].to_bits()
        }
    })
}

/// One triangle corner: its own position, then the positions of the next and
/// the previous corner. Draco can reorder vertices and faces and can rotate a
/// face, but it keeps the winding, so a corner keeps this name.
type CornerKey = [u32; 9];

/// The texture coordinates found at each named corner.
fn corner_uvs(positions: &[f32], uvs: &[f32], indices: &[u32]) -> HashMap<CornerKey, Vec<[f32; 2]>> {
    let mut corners: HashMap<CornerKey, Vec<[f32; 2]>> = HashMap::new();
    for face in indices.chunks_exact(3) {
        for c in 0..3 {
            let at = |i: usize| key(&positions[face[i] as usize * 3..][..3]);
            let mut name = [0; 9];
            name[..3].copy_from_slice(&at(c));
            name[3..6].copy_from_slice(&at((c + 1) % 3));
            name[6..].copy_from_slice(&at((c + 2) % 3));
            let v = face[c] as usize;
            corners
                .entry(name)
                .or_default()
                .push([uvs[2 * v], uvs[2 * v + 1]]);
        }
    }
    corners
}

/// The largest distance, in quantization steps, from each value in `from` to
/// the nearest value at the same corner in `to`. Panics when a corner of
/// `from` is missing in `to`.
fn worst_corner_error(
    from: &HashMap<CornerKey, Vec<[f32; 2]>>,
    to: &HashMap<CornerKey, Vec<[f32; 2]>>,
    step: f64,
    what: &str,
) -> f64 {
    let mut worst: f64 = 0.0;
    for (name, values) in from {
        let candidates = to
            .get(name)
            .unwrap_or_else(|| panic!("{what}: corner {name:?} has no match"));
        for value in values {
            let nearest = candidates
                .iter()
                .map(|other| {
                    (0..2)
                        .map(|i| (value[i] as f64 - other[i] as f64).abs())
                        .fold(0.0, f64::max)
                })
                .fold(f64::INFINITY, f64::min);
            worst = worst.max(nearest / step);
        }
    }
    worst
}

#[test]
#[ignore = "mesh grid corpus on .212 under /mnt/data2/gridcorpus/dump"]
fn grid_corpus_preserves_lattice_through_native_roundtrip() {
    let mut files: Vec<_> = std::fs::read_dir("/mnt/data2/gridcorpus/dump")
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "tfgd"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "required grid corpus is empty");
    let mut primitives = 0;
    let mut vertices = 0;
    let mut uv_primitives = 0;
    let mut uv_corners = 0;
    let mut worst_steps: Vec<f64> = UV_BITS.map(|_| 0.0).collect();
    let spacing = 1.0 / 256.0;
    for file in &files {
        let bytes = std::fs::read(file).unwrap();
        assert_eq!(&bytes[..4], b"TFGD");
        let count = u32_at(&bytes, 4);
        let mut offset = 8;
        for _ in 0..count {
            let nv = u32_at(&bytes, offset) as usize;
            let nt = u32_at(&bytes, offset + 4) as usize;
            let uv = bytes[offset + 8] != 0;
            offset += 12;
            let floats = |bytes: &[u8]| -> Vec<f32> {
                bytes
                    .chunks_exact(4)
                    .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
                    .collect()
            };
            let mut positions = floats(&bytes[offset..offset + 12 * nv]);
            offset += 12 * nv;
            let uvs = if uv {
                floats(&bytes[offset..offset + 8 * nv])
            } else {
                Vec::new()
            };
            offset += uvs.len() * 4;
            let indices: Vec<u32> = bytes[offset..offset + 12 * nt]
                .chunks_exact(4)
                .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
                .collect();
            offset += 12 * nt;
            if nv == 0 || nt == 0 {
                continue;
            }
            let before = positions.clone();
            snap_positions(&mut positions, spacing).unwrap();
            for (&a, &b) in positions.iter().zip(&before) {
                assert!((a as f64 - b as f64).abs() <= spacing as f64 / 2.0);
                if (b as f64 / spacing as f64).fract() == 0.0 {
                    assert_eq!(a, b);
                }
            }
            let snapped = positions.clone();
            snap_positions(&mut positions, spacing).unwrap();
            assert_eq!(positions, snapped);
            let expected: HashSet<_> = positions.chunks_exact(3).map(key).collect();
            let encoded = encode(
                MeshView {
                    attributes: &[Attribute::positions(&positions)],
                    indices: &indices,
                    num_vertices: nv,
                },
                &EncodeOptions {
                    position: Quantization::grid(spacing).unwrap(),
                    ..Default::default()
                },
            )
            .unwrap();
            let decoded = decode(&encoded.bytes).unwrap();
            let actual = decoded.read_f32(encoded.unique_ids[0]).unwrap();
            for point in actual.chunks_exact(3) {
                assert!(
                    expected.contains(&key(point)),
                    "decoded position moved off input grid: {point:?}, {}",
                    file.display()
                );
            }
            primitives += 1;
            vertices += nv;

            if !uv {
                continue;
            }
            // Both callers put texture coordinates inside the unit square on
            // the unit-square lattice.
            assert!(
                uvs.iter().all(|v| (0.0..=1.0).contains(v)),
                "texture coordinate outside the unit square: {}",
                file.display()
            );
            let input = corner_uvs(&positions, &uvs, &indices);
            for (bits, worst) in UV_BITS.zip(worst_steps.iter_mut()) {
                let encoded = encode(
                    MeshView {
                        attributes: &[
                            Attribute::positions(&positions),
                            Attribute::unit_tex_coords(&uvs, bits),
                        ],
                        indices: &indices,
                        num_vertices: nv,
                    },
                    &EncodeOptions {
                        position: Quantization::grid(spacing).unwrap(),
                        ..Default::default()
                    },
                )
                .unwrap();
                let decoded = decode(&encoded.bytes).unwrap();
                let output = corner_uvs(
                    &decoded.read_f32(encoded.unique_ids[0]).unwrap(),
                    &decoded.read_f32(encoded.unique_ids[1]).unwrap(),
                    &decoded.indices().unwrap(),
                );
                let step = 1.0 / ((1u64 << bits) - 1) as f64;
                let what = format!("{} at {bits} bits", file.display());
                let error = worst_corner_error(&output, &input, step, &what)
                    .max(worst_corner_error(&input, &output, step, &what));
                assert!(
                    error <= 1.0,
                    "{what}: a texture coordinate moved {error} quantization steps"
                );
                *worst = worst.max(error);
            }
            uv_primitives += 1;
            uv_corners += 3 * nt;
        }
        assert_eq!(offset, bytes.len());
    }
    eprintln!(
        "grid corpus: {} files, {primitives} primitives, {vertices} vertices",
        files.len()
    );
    eprintln!("texture coordinates: {uv_primitives} primitives, {uv_corners} corners per depth");
    for (bits, worst) in UV_BITS.zip(&worst_steps) {
        eprintln!("texture coordinates at {bits} bits: worst error {worst:.6} steps");
    }
}
