//! The raw C boundary. Nothing outside this module calls it.
//!
//! Every declaration here mirrors `csrc/tileforge_draco.cc`. Change one and
//! change the other.

use std::os::raw::c_char;

pub const TF_DRACO_OK: i32 = 0;

/// Mirrors `draco::GeometryAttribute::Type`.
pub const TF_DRACO_ATTR_POSITION: i32 = 0;
pub const TF_DRACO_ATTR_NORMAL: i32 = 1;
pub const TF_DRACO_ATTR_COLOR: i32 = 2;
pub const TF_DRACO_ATTR_TEX_COORD: i32 = 3;
pub const TF_DRACO_ATTR_GENERIC: i32 = 4;

#[repr(C)]
pub struct TfDracoAttribute {
    pub kind: i32,
    pub num_components: i32,
    pub quantization_bits: i32,
    pub data: *const f32,
    pub explicit_origin: *const f32,
    pub explicit_range: f32,
}

#[repr(C)]
pub struct TfDracoMesh {
    pub attributes: *const TfDracoAttribute,
    pub num_attributes: u32,
    pub indices: *const u32,
    pub num_vertices: u32,
    pub num_faces: u32,
}

#[repr(C)]
pub struct TfDracoEncodeOptions {
    pub position_spacing: f32,
    pub speed: i32,
}

#[repr(C)]
pub struct TfDracoBuffer {
    pub data: *mut u8,
    pub len: usize,
}

/// Opaque. Only `tf_draco_*` functions may touch one.
#[repr(C)]
pub struct TfDracoDecoded {
    _private: [u8; 0],
}

unsafe extern "C" {
    pub fn tf_draco_encode(
        mesh: *const TfDracoMesh,
        opts: *const TfDracoEncodeOptions,
        out: *mut TfDracoBuffer,
        out_unique_ids: *mut u32,
        err: *mut c_char,
        err_len: usize,
    ) -> i32;

    pub fn tf_draco_buffer_free(buf: *mut TfDracoBuffer);

    pub fn tf_draco_decode(
        data: *const u8,
        len: usize,
        out: *mut *mut TfDracoDecoded,
        err: *mut c_char,
        err_len: usize,
    ) -> i32;

    pub fn tf_draco_decoded_free(decoded: *mut TfDracoDecoded);

    pub fn tf_draco_decoded_num_points(decoded: *const TfDracoDecoded) -> u32;

    pub fn tf_draco_decoded_num_faces(decoded: *const TfDracoDecoded) -> u32;

    pub fn tf_draco_decoded_indices(decoded: *const TfDracoDecoded, out: *mut u32) -> i32;

    pub fn tf_draco_decoded_attribute(
        decoded: *const TfDracoDecoded,
        unique_id: u32,
        out_type: *mut i32,
        out_num_components: *mut i32,
    ) -> i32;

    pub fn tf_draco_decoded_read_f32(
        decoded: *const TfDracoDecoded,
        unique_id: u32,
        out: *mut f32,
        err: *mut c_char,
        err_len: usize,
    ) -> i32;

    pub fn tf_draco_decoded_read_u32(
        decoded: *const TfDracoDecoded,
        unique_id: u32,
        out: *mut u32,
        err: *mut c_char,
        err_len: usize,
    ) -> i32;
}
