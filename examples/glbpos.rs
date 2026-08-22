//! Print the decoded positions of a `KHR_draco_mesh_compression` GLB.
//!
//! The seam oracle needs to know where a tile's vertices land after the
//! decoder has run. Reading them through this crate means the oracle measures
//! the decoder we ship, not a second implementation of it.
//!
//! Usage: `glbpos <tile.glb> <out.f32>`
//!
//! The output is raw little-endian `f32`, three per vertex, with every
//! primitive of the file concatenated. Order is not meaningful; the oracle
//! matches by position.

use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: glbpos <tile.glb> <out.f32>");
        std::process::exit(2);
    }
    let glb = std::fs::read(&args[1]).expect("cannot read the GLB");
    let (json, bin) = split_glb(&glb);
    let doc: serde_json::Value =
        serde_json::from_slice(json).expect("the JSON chunk does not parse");

    let views = doc["bufferViews"].as_array().cloned().unwrap_or_default();
    let mut out: Vec<u8> = Vec::new();
    let mut vertices = 0usize;

    for mesh in doc["meshes"].as_array().into_iter().flatten() {
        for prim in mesh["primitives"].as_array().into_iter().flatten() {
            let Some(block) = prim
                .get("extensions")
                .and_then(|e| e.get("KHR_draco_mesh_compression"))
            else {
                continue;
            };
            let view = &views[block["bufferView"].as_u64().unwrap() as usize];
            let off = view["byteOffset"].as_u64().unwrap_or(0) as usize;
            let len = view["byteLength"].as_u64().unwrap() as usize;
            let id = block["attributes"]["POSITION"].as_u64().unwrap() as u32;

            let decoded = tileforge_draco::decode(&bin[off..off + len]).expect("Draco decode");
            let values = decoded.read_f32(id).expect("POSITION");
            vertices += decoded.num_points() as usize;
            for v in values {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
    std::fs::File::create(&args[2])
        .expect("cannot create the output")
        .write_all(&out)
        .expect("cannot write the output");
    println!("{vertices}");
}

/// Splits a GLB into its JSON chunk and its BIN chunk.
fn split_glb(glb: &[u8]) -> (&[u8], &[u8]) {
    assert_eq!(&glb[0..4], b"glTF", "not a GLB");
    let (mut json, mut bin): (&[u8], &[u8]) = (&[], &[]);
    let mut at = 12;
    while at + 8 <= glb.len() {
        let len = u32::from_le_bytes(glb[at..at + 4].try_into().unwrap()) as usize;
        let kind = u32::from_le_bytes(glb[at + 4..at + 8].try_into().unwrap());
        let body = &glb[at + 8..at + 8 + len];
        match kind {
            0x4E4F_534A => json = body,
            0x004E_4942 => bin = body,
            _ => {}
        }
        at += 8 + len;
    }
    (json, bin)
}
