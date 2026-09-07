//! Remote corpus gate for the shared snapping and native codec contract.
use std::collections::HashSet;
use tileforge_draco::{
    Attribute, EncodeOptions, MeshView, Quantization, decode, encode, snap_positions,
};

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
            let mut positions: Vec<f32> = bytes[offset..offset + 12 * nv]
                .chunks_exact(4)
                .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
                .collect();
            offset += 12 * nv + if uv { 8 * nv } else { 0 };
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
        }
        assert_eq!(offset, bytes.len());
    }
    eprintln!(
        "grid corpus: {} files, {primitives} primitives, {vertices} vertices",
        files.len()
    );
}
