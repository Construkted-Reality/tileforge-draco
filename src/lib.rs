//! Google Draco, for tileforge.
//!
//! # Why this crate replaced draco-oxide
//!
//! tileforge cuts one model into thousands of tiles and encodes each tile on
//! its own. Two neighbouring tiles hold the same vertex on their shared edge.
//! If the two encoders put that vertex in two different places, the render
//! shows a crack.
//!
//! Draco's `GLOBAL_GRID` quantization takes a grid **spacing** instead of a
//! bit count, and snaps every vertex to a grid anchored at zero. Every tile
//! therefore shares one lattice. draco-oxide has no such mode, and the
//! workaround we built for it cost 21 percent more bytes at the same step and
//! still broke the seam.
//!
//! Measured on a 932 tile corpus: with a power-of-two spacing and pre-snapped
//! input, 279,117 of 279,117 shared vertices stay bit-identical across all
//! 2,697 touching tile pairs. See
//! `docs/design/investigations/2026-08-21-draco-cpp-grid-validation.md`.
//!
//! # Two rules the measurement produced
//!
//! 1. Give [`Quantization::Grid`] a power-of-two spacing. Anything else halves
//!    the seam agreement and adds a drift of about 0.015 mm that no bit count
//!    removes. [`snap_positions`] and [`Quantization::grid`] both refuse a
//!    spacing that is not a power of two.
//! 2. Call [`snap_positions`] on the vertices first. Without it about 1 shared
//!    vertex in 5,000 lands one step out. With it, none do, and the output is
//!    slightly smaller.

mod ffi;

use std::ffi::CStr;
use std::os::raw::c_char;

const ERR_LEN: usize = 512;

/// What went wrong inside Draco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DracoError {
    /// The C entry point's status code.
    pub code: i32,
    /// Draco's own message, or ours when the argument check refused first.
    pub message: String,
}

impl std::fmt::Display for DracoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "draco error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for DracoError {}

/// How to quantize positions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Quantization {
    /// Snap to a grid of this spacing in metres, anchored at zero. Draco picks
    /// the bit count. Use this. It is the only mode that keeps two tiles on
    /// one lattice.
    Grid { spacing: f32 },
    /// Fit the mesh's own bounding box into this many bits. Every mesh gets a
    /// different lattice, so every shared vertex moves. Kept for measurement
    /// and for a caller that encodes a single whole model.
    Bits { bits: i32 },
}

impl Quantization {
    /// A grid whose spacing is a power of two, which is the only kind that
    /// decodes exactly.
    ///
    /// Draco decodes a position as `origin + index * step`, in 32-bit floats.
    /// When the spacing is a power of two, and the origin is a multiple of it,
    /// both the product and the sum are exact and two tiles must agree. When
    /// it is not, they disagree by about 0.015 mm.
    pub fn grid(spacing: f32) -> Result<Self, DracoError> {
        if !is_power_of_two(spacing) {
            return Err(DracoError {
                code: 1,
                message: format!(
                    "grid spacing {spacing} is not a power of two; \
                     a spacing that is not exactly representable puts \
                     neighbouring tiles on different lattices"
                ),
            });
        }
        Ok(Quantization::Grid { spacing })
    }
}

/// True when `v` is a positive, finite power of two.
pub fn is_power_of_two(v: f32) -> bool {
    v.is_finite() && v > 0.0 && v.to_bits() & 0x007f_ffff == 0
}

/// The largest power of two that is not larger than `target`.
///
/// Use it to turn a step derived from the data into one that decodes exactly.
/// It rounds down, never up, so the result is never coarser than asked.
pub fn power_of_two_at_most(target: f32) -> f32 {
    assert!(target.is_finite() && target > 0.0, "target must be positive");
    let mut v = f32::from_bits(target.to_bits() & 0xff80_0000);
    if v > target {
        v /= 2.0;
    }
    v
}

/// Rounds every position onto the global grid of `spacing`.
///
/// Draco quantizes as `floor((v - origin) / step + 0.5)`. That subtraction
/// rounds, and two neighbouring tiles have different origins, so a vertex
/// halfway between two grid points can round up in one tile and down in the
/// other. Pre-snapping makes the subtraction exact and removes the case.
///
/// Measured on corpus E: without this, 35 of 284,462 shared vertices land one
/// step out. With it, none do, and the corpus is 1,648 bytes smaller.
pub fn snap_positions(positions: &mut [f32], spacing: f32) -> Result<(), DracoError> {
    if !is_power_of_two(spacing) {
        return Err(DracoError {
            code: 1,
            message: format!("grid spacing {spacing} is not a power of two"),
        });
    }
    for v in positions.iter_mut() {
        *v = (*v / spacing + 0.5).floor() * spacing;
    }
    Ok(())
}

/// What kind of thing an attribute holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeType {
    Position,
    Normal,
    Color,
    TexCoord,
    Generic,
}

impl AttributeType {
    fn code(self) -> i32 {
        match self {
            AttributeType::Position => ffi::TF_DRACO_ATTR_POSITION,
            AttributeType::Normal => ffi::TF_DRACO_ATTR_NORMAL,
            AttributeType::Color => ffi::TF_DRACO_ATTR_COLOR,
            AttributeType::TexCoord => ffi::TF_DRACO_ATTR_TEX_COORD,
            AttributeType::Generic => ffi::TF_DRACO_ATTR_GENERIC,
        }
    }

    fn from_code(code: i32) -> Self {
        match code {
            ffi::TF_DRACO_ATTR_POSITION => AttributeType::Position,
            ffi::TF_DRACO_ATTR_NORMAL => AttributeType::Normal,
            ffi::TF_DRACO_ATTR_COLOR => AttributeType::Color,
            ffi::TF_DRACO_ATTR_TEX_COORD => AttributeType::TexCoord,
            _ => AttributeType::Generic,
        }
    }
}

/// One vertex attribute on its way into Draco.
///
/// `data` holds `components` floats per vertex, tightly packed, in vertex
/// order. Positions take their quantization from [`EncodeOptions::position`],
/// so `quantization_bits` is ignored on a position attribute that is on a grid.
#[derive(Debug, Clone, Copy)]
pub struct Attribute<'a> {
    pub kind: AttributeType,
    pub components: usize,
    /// 0 leaves the attribute unquantized.
    pub quantization_bits: i32,
    pub data: &'a [f32],
}

impl<'a> Attribute<'a> {
    /// Positions, three floats per vertex.
    pub fn positions(data: &'a [f32]) -> Self {
        Self {
            kind: AttributeType::Position,
            components: 3,
            quantization_bits: 0,
            data,
        }
    }

    /// Texture coordinates, two floats per vertex.
    pub fn tex_coords(data: &'a [f32], bits: i32) -> Self {
        Self {
            kind: AttributeType::TexCoord,
            components: 2,
            quantization_bits: bits,
            data,
        }
    }
}

/// One mesh on its way into Draco.
///
/// Exactly one attribute must be a [`AttributeType::Position`]. `indices` holds
/// three entries per triangle.
#[derive(Debug, Clone, Copy)]
pub struct MeshView<'a> {
    pub attributes: &'a [Attribute<'a>],
    pub indices: &'a [u32],
    pub num_vertices: usize,
}

/// Encoder settings.
#[derive(Debug, Clone, Copy)]
pub struct EncodeOptions {
    pub position: Quantization,
    /// Draco's own scale: 0 is slowest and smallest, 10 is fastest.
    pub speed: i32,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            position: Quantization::Bits { bits: 14 },
            speed: 0,
        }
    }
}

/// A Draco bitstream and the unique id Draco gave each input attribute.
///
/// The `KHR_draco_mesh_compression` extension names attributes by unique id,
/// not by position, so the caller needs both halves to write the extension.
#[derive(Debug, Clone)]
pub struct Encoded {
    pub bytes: Vec<u8>,
    /// One id per attribute of the input mesh, in the same order.
    pub unique_ids: Vec<u32>,
}

/// Compresses one mesh.
pub fn encode(mesh: MeshView<'_>, opts: &EncodeOptions) -> Result<Encoded, DracoError> {
    let num_vertices = mesh.num_vertices;
    if mesh.indices.len() % 3 != 0 {
        return Err(argument("indices is not a multiple of 3"));
    }
    if num_vertices == 0 || mesh.indices.is_empty() {
        return Err(argument("mesh has no vertices or no triangles"));
    }
    if mesh
        .attributes
        .iter()
        .filter(|a| a.kind == AttributeType::Position)
        .count()
        != 1
    {
        return Err(argument("a mesh needs exactly one position attribute"));
    }
    for att in mesh.attributes {
        if att.components < 1 || att.components > 4 {
            return Err(argument("an attribute needs 1 to 4 components"));
        }
        if att.data.len() != num_vertices * att.components {
            return Err(argument(&format!(
                "attribute {:?} holds {} floats, not {} vertices times {} components",
                att.kind,
                att.data.len(),
                num_vertices,
                att.components
            )));
        }
    }
    if let Some(&bad) = mesh.indices.iter().find(|&&i| i as usize >= num_vertices) {
        return Err(argument(&format!(
            "index {bad} is past the last of {num_vertices} vertices"
        )));
    }

    let position_spacing = match opts.position {
        Quantization::Grid { spacing } => {
            if !is_power_of_two(spacing) {
                return Err(argument(&format!(
                    "grid spacing {spacing} is not a power of two"
                )));
            }
            spacing
        }
        Quantization::Bits { bits } => {
            if bits <= 0 {
                return Err(argument("a bit count must be positive"));
            }
            0.0
        }
    };
    let position_bits = match opts.position {
        Quantization::Bits { bits } => bits,
        Quantization::Grid { .. } => 0,
    };

    let c_atts: Vec<ffi::TfDracoAttribute> = mesh
        .attributes
        .iter()
        .map(|a| ffi::TfDracoAttribute {
            kind: a.kind.code(),
            num_components: a.components as i32,
            quantization_bits: if a.kind == AttributeType::Position {
                position_bits
            } else {
                a.quantization_bits
            },
            data: a.data.as_ptr(),
        })
        .collect();

    let c_mesh = ffi::TfDracoMesh {
        attributes: c_atts.as_ptr(),
        num_attributes: c_atts.len() as u32,
        indices: mesh.indices.as_ptr(),
        num_vertices: num_vertices as u32,
        num_faces: (mesh.indices.len() / 3) as u32,
    };
    let c_opts = ffi::TfDracoEncodeOptions {
        position_spacing,
        speed: opts.speed,
    };
    let mut out = ffi::TfDracoBuffer {
        data: std::ptr::null_mut(),
        len: 0,
    };
    let mut unique_ids = vec![0u32; c_atts.len()];
    let mut err = [0 as c_char; ERR_LEN];

    // SAFETY: every pointer above comes from a live slice that outlives the
    // call, `unique_ids` holds one entry per attribute, and `out` and `err` are
    // owned here.
    let code = unsafe {
        ffi::tf_draco_encode(
            &c_mesh,
            &c_opts,
            &mut out,
            unique_ids.as_mut_ptr(),
            err.as_mut_ptr(),
            ERR_LEN,
        )
    };
    if code != ffi::TF_DRACO_OK {
        return Err(DracoError {
            code,
            message: read_err(&err),
        });
    }

    // SAFETY: on success the C side handed us `len` initialised bytes.
    let bytes = unsafe { std::slice::from_raw_parts(out.data, out.len) }.to_vec();
    // SAFETY: `out` is the buffer the C side allocated and we have copied it.
    unsafe { ffi::tf_draco_buffer_free(&mut out) };
    Ok(Encoded { bytes, unique_ids })
}

/// A decoded Draco mesh. Frees the C++ object when dropped.
pub struct DecodedMesh {
    raw: *mut ffi::TfDracoDecoded,
}

// SAFETY: the handle owns its C++ object exclusively and the C entry point
// holds no global state.
unsafe impl Send for DecodedMesh {}

impl Drop for DecodedMesh {
    fn drop(&mut self) {
        // SAFETY: `raw` came from `tf_draco_decode` and is freed once.
        unsafe { ffi::tf_draco_decoded_free(self.raw) };
    }
}

impl DecodedMesh {
    pub fn num_points(&self) -> u32 {
        // SAFETY: `raw` is live for the lifetime of self.
        unsafe { ffi::tf_draco_decoded_num_points(self.raw) }
    }

    pub fn num_faces(&self) -> u32 {
        // SAFETY: as above.
        unsafe { ffi::tf_draco_decoded_num_faces(self.raw) }
    }

    /// Triangle indices, three per face.
    pub fn indices(&self) -> Result<Vec<u32>, DracoError> {
        let mut out = vec![0u32; self.num_faces() as usize * 3];
        // SAFETY: `out` holds exactly the count the C side writes.
        let code = unsafe { ffi::tf_draco_decoded_indices(self.raw, out.as_mut_ptr()) };
        if code != ffi::TF_DRACO_OK {
            return Err(DracoError {
                code,
                message: "cannot read indices".to_string(),
            });
        }
        Ok(out)
    }

    /// The type and component count of the attribute with this Draco unique
    /// id. The `KHR_draco_mesh_compression` extension names attributes by that
    /// id, so this is how "POSITION" or "TEXCOORD_0" finds its data.
    pub fn attribute(&self, unique_id: u32) -> Option<(AttributeType, usize)> {
        let mut kind = 0i32;
        let mut components = 0i32;
        // SAFETY: both out parameters are owned here.
        let code = unsafe {
            ffi::tf_draco_decoded_attribute(self.raw, unique_id, &mut kind, &mut components)
        };
        if code != ffi::TF_DRACO_OK {
            return None;
        }
        Some((AttributeType::from_code(kind), components as usize))
    }

    /// Reads an attribute as floats, one entry per point, in point order.
    pub fn read_f32(&self, unique_id: u32) -> Result<Vec<f32>, DracoError> {
        let components = self.components_or_err(unique_id)?;
        let mut out = vec![0f32; self.num_points() as usize * components];
        let mut err = [0 as c_char; ERR_LEN];
        // SAFETY: `out` holds num_points * components floats, which is what
        // the C side writes.
        let code = unsafe {
            ffi::tf_draco_decoded_read_f32(
                self.raw,
                unique_id,
                out.as_mut_ptr(),
                err.as_mut_ptr(),
                ERR_LEN,
            )
        };
        if code != ffi::TF_DRACO_OK {
            return Err(DracoError {
                code,
                message: read_err(&err),
            });
        }
        Ok(out)
    }

    /// The same, as unsigned 32-bit integers.
    pub fn read_u32(&self, unique_id: u32) -> Result<Vec<u32>, DracoError> {
        let components = self.components_or_err(unique_id)?;
        let mut out = vec![0u32; self.num_points() as usize * components];
        let mut err = [0 as c_char; ERR_LEN];
        // SAFETY: as in read_f32.
        let code = unsafe {
            ffi::tf_draco_decoded_read_u32(
                self.raw,
                unique_id,
                out.as_mut_ptr(),
                err.as_mut_ptr(),
                ERR_LEN,
            )
        };
        if code != ffi::TF_DRACO_OK {
            return Err(DracoError {
                code,
                message: read_err(&err),
            });
        }
        Ok(out)
    }

    fn components_or_err(&self, unique_id: u32) -> Result<usize, DracoError> {
        self.attribute(unique_id)
            .map(|(_, c)| c)
            .ok_or_else(|| argument(&format!("no attribute with unique id {unique_id}")))
    }
}

/// Decompresses a Draco bitstream.
///
/// This does not bound the output. A hostile bitstream declares its own vertex
/// and face counts, and Draco sizes its buffers from them. Check the expansion
/// ratio before you call this on anything a user supplied.
pub fn decode(data: &[u8]) -> Result<DecodedMesh, DracoError> {
    if data.is_empty() {
        return Err(argument("empty bitstream"));
    }
    let mut raw: *mut ffi::TfDracoDecoded = std::ptr::null_mut();
    let mut err = [0 as c_char; ERR_LEN];
    // SAFETY: `data` outlives the call, and `raw` and `err` are owned here.
    let code =
        unsafe { ffi::tf_draco_decode(data.as_ptr(), data.len(), &mut raw, err.as_mut_ptr(), ERR_LEN) };
    if code != ffi::TF_DRACO_OK || raw.is_null() {
        return Err(DracoError {
            code,
            message: read_err(&err),
        });
    }
    Ok(DecodedMesh { raw })
}

fn argument(message: &str) -> DracoError {
    DracoError {
        code: 1,
        message: message.to_string(),
    }
}

fn read_err(buf: &[c_char; ERR_LEN]) -> String {
    // SAFETY: the C side always writes a terminator inside the buffer.
    unsafe { CStr::from_ptr(buf.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two quads that share an edge at x = 4, in the metre range where the
    /// Draco option-store defect used to corrupt the grid origin.
    fn quad(x0: f32, x1: f32) -> (Vec<f32>, Vec<u32>) {
        let positions = vec![
            x0, 0.0, 0.0, //
            x1, 0.0, 0.0, //
            x1, 1.0, 0.0, //
            x0, 1.0, 0.0, //
        ];
        (positions, vec![0, 1, 2, 0, 2, 3])
    }

    /// Encodes a positions-and-indices pair with the given settings.
    fn encode_quad(m: &(Vec<f32>, Vec<u32>), opts: &EncodeOptions) -> Encoded {
        encode(
            MeshView {
                attributes: &[Attribute::positions(&m.0)],
                indices: &m.1,
                num_vertices: m.0.len() / 3,
            },
            opts,
        )
        .unwrap()
    }

    #[test]
    fn a_mesh_round_trips() {
        let (positions, indices) = quad(0.0, 4.0);
        let encoded = encode_quad(
            &(positions, indices),
            &EncodeOptions {
                position: Quantization::grid(0.00390625).unwrap(),
                speed: 0,
            },
        );
        assert!(!encoded.bytes.is_empty());
        assert_eq!(encoded.unique_ids, vec![0]);

        let decoded = decode(&encoded.bytes).unwrap();
        assert_eq!(decoded.num_faces(), 2);
        let (kind, components) = decoded.attribute(0).unwrap();
        assert_eq!(kind, AttributeType::Position);
        assert_eq!(components, 3);
        assert_eq!(decoded.indices().unwrap().len(), 6);
        assert_eq!(decoded.read_f32(0).unwrap().len(), decoded.num_points() as usize * 3);
    }

    #[test]
    fn neighbouring_tiles_agree_on_a_shared_edge() {
        // The whole reason this crate exists. Two tiles meet at x = 4. The
        // vertices on that edge must decode to the same place in both.
        //
        // 4 metres matters: it is inside the range where Draco's own option
        // store used to lose the low bits of the grid origin. See
        // third_party/draco/CONSTRUKTED-CHANGES.md.
        let spacing = 0.00390625;
        let mut left = quad(-3.0, 4.0);
        let mut right = quad(4.0, 11.0);
        snap_positions(&mut left.0, spacing).unwrap();
        snap_positions(&mut right.0, spacing).unwrap();

        let opts = EncodeOptions {
            position: Quantization::grid(spacing).unwrap(),
            speed: 0,
        };
        let decode_one = |m: &(Vec<f32>, Vec<u32>)| {
            decode(&encode_quad(m, &opts).bytes)
                .unwrap()
                .read_f32(0)
                .unwrap()
        };
        let dl = decode_one(&left);
        let dr = decode_one(&right);

        // Every vertex at x = 4 must be bit-identical in both tiles.
        let on_seam = |v: &[f32]| -> Vec<[f32; 3]> {
            let mut out: Vec<[f32; 3]> = v
                .chunks_exact(3)
                .filter(|p| p[0] == 4.0)
                .map(|p| [p[0], p[1], p[2]])
                .collect();
            out.sort_by(|a, b| a.partial_cmp(b).unwrap());
            out.dedup();
            out
        };
        let sl = on_seam(&dl);
        let sr = on_seam(&dr);
        assert_eq!(sl.len(), 2, "the left tile lost its seam vertices");
        assert_eq!(sl, sr, "the two tiles disagree about the shared edge");
    }

    #[test]
    fn a_bit_count_moves_a_shared_vertex_that_is_not_on_a_corner() {
        // The counterpart. Per-mesh bit quantization gives each tile its own
        // bounding box, so a shared vertex lands in two different places.
        //
        // Two things make this harder to show than it looks.
        //
        // 1. Draco maps a bounding-box minimum to index 0 and a maximum to
        //    the last index, so a vertex on a corner of both boxes survives
        //    even per-mesh quantization. The vertex has to be an interior one.
        // 2. Draco's range is a single scalar, the largest span over the three
        //    axes, so the quantization cell is a cube. Two tiles with the same
        //    largest span therefore get the same step and still agree. The two
        //    tiles below have different largest spans, 2 and 7.
        let seam_y = 1.0f32;
        let left = (
            vec![
                2.0, 0.0, 0.0, //
                4.0, 0.0, 0.0, //
                4.0, seam_y, 0.0, // on the seam, interior in y for the right tile
                4.0, 2.0, 0.0, //
                2.0, 2.0, 0.0, //
            ],
            vec![0, 1, 2, 0, 2, 3, 0, 3, 4],
        );
        let right = (
            vec![
                4.0, 0.0, 0.0, //
                11.0, 0.0, 0.0, //
                11.0, 7.0, 0.0, //
                4.0, 7.0, 0.0, //
                4.0, seam_y, 0.0, // the same vertex, interior in this tile
            ],
            vec![0, 1, 2, 0, 2, 3, 0, 4, 3],
        );

        let opts = EncodeOptions {
            position: Quantization::Bits { bits: 8 },
            speed: 0,
        };
        let decode_one = |m: &(Vec<f32>, Vec<u32>)| {
            decode(&encode_quad(m, &opts).bytes)
                .unwrap()
                .read_f32(0)
                .unwrap()
        };
        let dl = decode_one(&left);
        let dr = decode_one(&right);

        // Find where each tile put the vertex that was at (4, seam_y, 0).
        let nearest_y = |v: &[f32]| -> f32 {
            v.chunks_exact(3)
                .filter(|p| p[0] > 3.9)
                .map(|p| p[1])
                .min_by(|a, b| {
                    (a - seam_y)
                        .abs()
                        .partial_cmp(&(b - seam_y).abs())
                        .unwrap()
                })
                .unwrap()
        };
        assert_ne!(
            nearest_y(&dl),
            nearest_y(&dr),
            "per-mesh quantization is expected to move an interior shared vertex"
        );
    }

    #[test]
    fn a_grid_holds_the_same_vertex_that_a_bit_count_moves() {
        // The same geometry as the test above, on a grid. Both tiles must put
        // the shared vertex in one place.
        let spacing = 0.00390625;
        let seam_y = 1.0f32;
        let mut left = (
            vec![
                2.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0, seam_y, 0.0, 4.0, 2.0, 0.0, 2.0, 2.0, 0.0,
            ],
            vec![0u32, 1, 2, 0, 2, 3, 0, 3, 4],
        );
        let mut right = (
            vec![
                4.0, 0.0, 0.0, 11.0, 0.0, 0.0, 11.0, 7.0, 0.0, 4.0, 7.0, 0.0, 4.0, seam_y, 0.0,
            ],
            vec![0u32, 1, 2, 0, 2, 3, 0, 4, 3],
        );
        snap_positions(&mut left.0, spacing).unwrap();
        snap_positions(&mut right.0, spacing).unwrap();

        let opts = EncodeOptions {
            position: Quantization::grid(spacing).unwrap(),
            speed: 0,
        };
        let decode_one = |m: &(Vec<f32>, Vec<u32>)| {
            decode(&encode_quad(m, &opts).bytes)
                .unwrap()
                .read_f32(0)
                .unwrap()
        };
        let dl = decode_one(&left);
        let dr = decode_one(&right);
        let holds = |v: &[f32]| v.chunks_exact(3).any(|p| p == [4.0, seam_y, 0.0]);
        assert!(holds(&dl), "the left tile moved the shared vertex");
        assert!(holds(&dr), "the right tile moved the shared vertex");
    }

    #[test]
    fn every_attribute_comes_back_under_its_own_unique_id() {
        // The glTF KHR_draco_mesh_compression extension names attributes by
        // Draco unique id. A tile carries positions, texture coordinates and
        // sometimes normals, so the encoder has to report an id for each one
        // and the decoder has to find each one again.
        let (positions, indices) = quad(0.0, 4.0);
        let uvs = vec![0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0];
        let normals = vec![
            0.0, 0.0, 1.0, //
            0.0, 0.0, 1.0, //
            0.0, 0.0, 1.0, //
            0.0, 0.0, 1.0, //
        ];
        let encoded = encode(
            MeshView {
                attributes: &[
                    Attribute::positions(&positions),
                    Attribute::tex_coords(&uvs, 12),
                    Attribute {
                        kind: AttributeType::Normal,
                        components: 3,
                        quantization_bits: 10,
                        data: &normals,
                    },
                ],
                indices: &indices,
                num_vertices: 4,
            },
            &EncodeOptions {
                position: Quantization::grid(0.00390625).unwrap(),
                speed: 0,
            },
        )
        .unwrap();

        assert_eq!(encoded.unique_ids.len(), 3);
        let mut sorted = encoded.unique_ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 3, "two attributes share a unique id");

        let decoded = decode(&encoded.bytes).unwrap();
        let expected = [
            (AttributeType::Position, 3),
            (AttributeType::TexCoord, 2),
            (AttributeType::Normal, 3),
        ];
        for (id, (kind, components)) in encoded.unique_ids.iter().zip(expected) {
            let got = decoded.attribute(*id).expect("unique id is not in the bitstream");
            assert_eq!(got, (kind, components), "unique id {id}");
            assert_eq!(
                decoded.read_f32(*id).unwrap().len(),
                decoded.num_points() as usize * components
            );
        }
    }

    #[test]
    fn a_mesh_needs_exactly_one_position_attribute() {
        let uvs = vec![0.0, 0.0, 1.0, 0.0, 1.0, 1.0];
        let err = encode(
            MeshView {
                attributes: &[Attribute::tex_coords(&uvs, 12)],
                indices: &[0, 1, 2],
                num_vertices: 3,
            },
            &EncodeOptions::default(),
        )
        .unwrap_err();
        assert!(err.message.contains("exactly one position"), "{err}");
    }

    #[test]
    fn an_attribute_of_the_wrong_length_is_refused() {
        let (positions, indices) = quad(0.0, 4.0);
        let uvs = vec![0.0, 0.0, 1.0, 0.0];  // 2 vertices, not 4
        let err = encode(
            MeshView {
                attributes: &[
                    Attribute::positions(&positions),
                    Attribute::tex_coords(&uvs, 12),
                ],
                indices: &indices,
                num_vertices: 4,
            },
            &EncodeOptions::default(),
        )
        .unwrap_err();
        assert!(err.message.contains("holds 4 floats"), "{err}");
    }

    #[test]
    fn a_grid_spacing_must_be_a_power_of_two() {
        assert!(Quantization::grid(0.003318049).is_err());
        assert!(Quantization::grid(0.00390625).is_ok());
        assert!(snap_positions(&mut [1.0], 0.003318049).is_err());
    }

    #[test]
    fn power_of_two_at_most_rounds_down() {
        assert_eq!(power_of_two_at_most(0.003318049), 0.001953125);
        assert_eq!(power_of_two_at_most(0.00390625), 0.00390625);
        assert_eq!(power_of_two_at_most(3.0), 2.0);
        assert!(is_power_of_two(power_of_two_at_most(0.1)));
    }

    #[test]
    fn snapping_puts_every_vertex_on_the_grid() {
        let spacing = 0.00390625;
        let mut v = vec![4.369245529174805, -49.99343490600586, 0.0007];
        snap_positions(&mut v, spacing).unwrap();
        for x in &v {
            assert_eq!(x / spacing, (x / spacing).round(), "{x} is off the grid");
        }
    }

    #[test]
    fn an_empty_mesh_is_refused_with_a_clear_message() {
        // Corpus E holds two tiles with vertices and no triangles. Draco says
        // "All triangles are degenerate", which reads like a geometry fault
        // and is not one.
        let err = encode(
            MeshView {
                attributes: &[Attribute::positions(&[0.0, 0.0, 0.0])],
                indices: &[],
                num_vertices: 1,
            },
            &EncodeOptions::default(),
        )
        .unwrap_err();
        assert!(err.message.contains("no vertices or no triangles"), "{err}");
    }

    #[test]
    fn an_index_past_the_end_is_refused() {
        let err = encode(
            MeshView {
                attributes: &[Attribute::positions(&[
                    0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0,
                ])],
                indices: &[0, 1, 9],
                num_vertices: 3,
            },
            &EncodeOptions::default(),
        )
        .unwrap_err();
        assert!(err.message.contains("past the last"), "{err}");
    }

    #[test]
    fn a_truncated_bitstream_is_refused_rather_than_trusted() {
        let bytes = encode_quad(&quad(0.0, 4.0), &EncodeOptions::default()).bytes;
        assert!(decode(&bytes[..bytes.len() / 2]).is_err());
        assert!(decode(&[]).is_err());
    }
}
