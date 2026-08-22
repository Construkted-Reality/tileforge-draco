// A C entry point onto Google Draco, for tileforge.
//
// Why this file exists at all:
//
// 1. Draco's own glTF front end cannot read a tileforge tile. It rejects
//    `KHR_texture_basisu`, which every tile requires. So we do not hand Draco
//    a glTF file. We hand it vertex buffers.
// 2. The `draco_transcoder` command line exposes only a bit count. The grid
//    quantization that keeps neighbouring tiles on one lattice needs a
//    spacing, and only the C++ API takes one.
//
// Everything here is plain C at the boundary. The Rust side owns no C++
// object and never sees a C++ exception.
//
// See docs/design/investigations/2026-08-21-draco-cpp-grid-validation.md.

#include <cstdint>
#include <cstring>
#include <memory>
#include <new>
#include <string>
#include <vector>

#include "draco/compression/decode.h"
#include "draco/compression/expert_encode.h"
#include "draco/mesh/mesh.h"

extern "C" {

// Keep these in step with src/ffi.rs.
#define TF_DRACO_OK 0
#define TF_DRACO_ERR_ARGUMENT 1
#define TF_DRACO_ERR_ENCODE 2
#define TF_DRACO_ERR_DECODE 3
#define TF_DRACO_ERR_UNSUPPORTED 4
#define TF_DRACO_ERR_INTERNAL 5

// Mirrors draco::GeometryAttribute::Type. Repeated rather than included so
// the Rust side has one place to read.
#define TF_DRACO_ATTR_INVALID -1
#define TF_DRACO_ATTR_POSITION 0
#define TF_DRACO_ATTR_NORMAL 1
#define TF_DRACO_ATTR_COLOR 2
#define TF_DRACO_ATTR_TEX_COORD 3
#define TF_DRACO_ATTR_GENERIC 4

// One vertex attribute on its way in. |data| holds num_components floats per
// vertex, tightly packed, in vertex order.
struct TfDracoAttribute {
  int32_t type;            // one of TF_DRACO_ATTR_*
  int32_t num_components;  // 1 to 4
  int32_t quantization_bits;  // 0 leaves the attribute unquantized
  const float *data;
};

struct TfDracoMesh {
  const TfDracoAttribute *attributes;  // POSITION must be one of them
  uint32_t num_attributes;
  const uint32_t *indices;  // 3 * num_faces, never null
  uint32_t num_vertices;
  uint32_t num_faces;
};

struct TfDracoEncodeOptions {
  // When > 0, quantize positions onto the global grid of this spacing. Draco
  // then chooses the bit count itself. This is the mode that keeps two
  // neighbouring tiles on one lattice, and it overrides the POSITION
  // attribute's own quantization_bits.
  float position_spacing;
  // Draco's own scale, 0 slowest and smallest, 10 fastest and largest.
  int32_t speed;
};

struct TfDracoBuffer {
  uint8_t *data;
  size_t len;
};

struct TfDracoDecoded {
  std::unique_ptr<draco::Mesh> mesh;
};

namespace {

void SetError(char *err, size_t err_len, const std::string &msg) {
  if (err == nullptr || err_len == 0) {
    return;
  }
  const size_t n = msg.size() < err_len - 1 ? msg.size() : err_len - 1;
  memcpy(err, msg.data(), n);
  err[n] = '\0';
}

// Builds the Draco mesh and records, per input attribute, the unique id Draco
// gave it. The glTF KHR_draco_mesh_compression extension names attributes by
// that id, so the caller needs it back.
std::unique_ptr<draco::Mesh> BuildMesh(const TfDracoMesh &in,
                                       std::vector<int> *att_ids) {
  std::unique_ptr<draco::Mesh> mesh(new draco::Mesh());
  mesh->set_num_points(in.num_vertices);
  mesh->SetNumFaces(in.num_faces);

  att_ids->clear();
  for (uint32_t a = 0; a < in.num_attributes; ++a) {
    const TfDracoAttribute &src = in.attributes[a];
    const int nc = src.num_components;
    draco::GeometryAttribute att;
    att.Init(static_cast<draco::GeometryAttribute::Type>(src.type), nullptr, nc,
             draco::DT_FLOAT32, false, sizeof(float) * nc, 0);
    const int id = mesh->AddAttribute(att, true, in.num_vertices);
    for (uint32_t v = 0; v < in.num_vertices; ++v) {
      mesh->attribute(id)->SetAttributeValue(draco::AttributeValueIndex(v),
                                             &src.data[nc * v]);
    }
    att_ids->push_back(id);
  }

  for (uint32_t f = 0; f < in.num_faces; ++f) {
    draco::Mesh::Face face;
    for (int c = 0; c < 3; ++c) {
      face[c] = draco::PointIndex(in.indices[3 * f + c]);
    }
    mesh->SetFace(draco::FaceIndex(f), face);
  }
  return mesh;
}

}  // namespace

// Encodes one mesh. On success the caller owns |out| and must release it with
// tf_draco_buffer_free. |out_unique_ids| receives one unique id per input
// attribute, in the same order, and must hold num_attributes entries.
int32_t tf_draco_encode(const TfDracoMesh *in, const TfDracoEncodeOptions *opts,
                        TfDracoBuffer *out, uint32_t *out_unique_ids, char *err,
                        size_t err_len) {
  if (in == nullptr || opts == nullptr || out == nullptr ||
      in->attributes == nullptr || in->indices == nullptr) {
    SetError(err, err_len, "null argument");
    return TF_DRACO_ERR_ARGUMENT;
  }
  if (in->num_vertices == 0 || in->num_faces == 0) {
    // Draco refuses a mesh with no faces, and the caller must not send one.
    // Report it rather than let Draco return "All triangles are degenerate",
    // which reads like a geometry problem and is not one.
    SetError(err, err_len, "mesh has no vertices or no faces");
    return TF_DRACO_ERR_ARGUMENT;
  }
  for (uint32_t a = 0; a < in->num_attributes; ++a) {
    const TfDracoAttribute &att = in->attributes[a];
    if (att.data == nullptr || att.num_components < 1 ||
        att.num_components > 4) {
      SetError(err, err_len, "attribute has no data or a bad component count");
      return TF_DRACO_ERR_ARGUMENT;
    }
  }

  std::vector<int> att_ids;
  std::unique_ptr<draco::Mesh> mesh;
  try {
    mesh = BuildMesh(*in, &att_ids);
  } catch (const std::bad_alloc &) {
    SetError(err, err_len, "out of memory while building the mesh");
    return TF_DRACO_ERR_INTERNAL;
  }

  draco::ExpertEncoder encoder(*mesh);
  encoder.SetSpeedOptions(opts->speed, opts->speed);

  bool saw_position = false;
  for (uint32_t a = 0; a < in->num_attributes; ++a) {
    const TfDracoAttribute &src = in->attributes[a];
    const int id = att_ids[a];
    if (src.type == TF_DRACO_ATTR_POSITION && opts->position_spacing > 0.f) {
      saw_position = true;
      const draco::Status s =
          encoder.SetAttributeGridQuantization(*mesh, id,
                                               opts->position_spacing);
      if (!s.ok()) {
        SetError(err, err_len,
                 std::string("grid quantization: ") + s.error_msg());
        return TF_DRACO_ERR_ENCODE;
      }
    } else if (src.quantization_bits > 0) {
      if (src.type == TF_DRACO_ATTR_POSITION) {
        saw_position = true;
      }
      encoder.SetAttributeQuantization(id, src.quantization_bits);
    }
  }
  if (!saw_position) {
    SetError(err, err_len,
             "positions have neither a grid spacing nor a bit count");
    return TF_DRACO_ERR_ARGUMENT;
  }

  draco::EncoderBuffer buf;
  const draco::Status s = encoder.EncodeToBuffer(&buf);
  if (!s.ok()) {
    SetError(err, err_len, std::string("encode: ") + s.error_msg());
    return TF_DRACO_ERR_ENCODE;
  }

  if (out_unique_ids != nullptr) {
    for (uint32_t a = 0; a < in->num_attributes; ++a) {
      out_unique_ids[a] = mesh->attribute(att_ids[a])->unique_id();
    }
  }

  out->len = buf.size();
  out->data = static_cast<uint8_t *>(malloc(out->len));
  if (out->data == nullptr) {
    out->len = 0;
    SetError(err, err_len, "out of memory while copying the encoded buffer");
    return TF_DRACO_ERR_INTERNAL;
  }
  memcpy(out->data, buf.data(), out->len);
  return TF_DRACO_OK;
}

void tf_draco_buffer_free(TfDracoBuffer *buf) {
  if (buf == nullptr || buf->data == nullptr) {
    return;
  }
  free(buf->data);
  buf->data = nullptr;
  buf->len = 0;
}

// Decodes a Draco mesh. On success the caller owns |*out| and must release it
// with tf_draco_decoded_free.
int32_t tf_draco_decode(const uint8_t *data, size_t len, TfDracoDecoded **out,
                        char *err, size_t err_len) {
  if (data == nullptr || out == nullptr) {
    SetError(err, err_len, "null argument");
    return TF_DRACO_ERR_ARGUMENT;
  }
  *out = nullptr;

  draco::DecoderBuffer buffer;
  buffer.Init(reinterpret_cast<const char *>(data), len);

  const auto type = draco::Decoder::GetEncodedGeometryType(&buffer);
  if (!type.ok()) {
    SetError(err, err_len, std::string("geometry type: ") +
                               type.status().error_msg());
    return TF_DRACO_ERR_DECODE;
  }
  if (type.value() != draco::TRIANGULAR_MESH) {
    SetError(err, err_len, "not a triangular mesh");
    return TF_DRACO_ERR_UNSUPPORTED;
  }

  draco::Decoder decoder;
  auto maybe = decoder.DecodeMeshFromBuffer(&buffer);
  if (!maybe.ok()) {
    SetError(err, err_len,
             std::string("decode: ") + maybe.status().error_msg());
    return TF_DRACO_ERR_DECODE;
  }

  TfDracoDecoded *decoded = new (std::nothrow) TfDracoDecoded();
  if (decoded == nullptr) {
    SetError(err, err_len, "out of memory");
    return TF_DRACO_ERR_INTERNAL;
  }
  decoded->mesh = std::move(maybe).value();
  *out = decoded;
  return TF_DRACO_OK;
}

void tf_draco_decoded_free(TfDracoDecoded *decoded) { delete decoded; }

uint32_t tf_draco_decoded_num_points(const TfDracoDecoded *decoded) {
  return decoded == nullptr ? 0 : decoded->mesh->num_points();
}

uint32_t tf_draco_decoded_num_faces(const TfDracoDecoded *decoded) {
  return decoded == nullptr ? 0 : decoded->mesh->num_faces();
}

// Writes 3 * num_faces indices. |out| must hold that many.
int32_t tf_draco_decoded_indices(const TfDracoDecoded *decoded, uint32_t *out) {
  if (decoded == nullptr || out == nullptr) {
    return TF_DRACO_ERR_ARGUMENT;
  }
  const draco::Mesh &mesh = *decoded->mesh;
  for (draco::FaceIndex f(0); f < mesh.num_faces(); ++f) {
    const draco::Mesh::Face &face = mesh.face(f);
    for (int c = 0; c < 3; ++c) {
      out[3 * f.value() + c] = face[c].value();
    }
  }
  return TF_DRACO_OK;
}

// Describes the attribute Draco gave the unique id |unique_id|. The glTF
// KHR_draco_mesh_compression extension names attributes by that id, so this is
// how a caller maps "POSITION" or "TEXCOORD_0" onto decoded data.
int32_t tf_draco_decoded_attribute(const TfDracoDecoded *decoded,
                                   uint32_t unique_id, int32_t *out_type,
                                   int32_t *out_num_components) {
  if (decoded == nullptr || out_type == nullptr ||
      out_num_components == nullptr) {
    return TF_DRACO_ERR_ARGUMENT;
  }
  const draco::PointAttribute *att =
      decoded->mesh->GetAttributeByUniqueId(unique_id);
  if (att == nullptr) {
    *out_type = TF_DRACO_ATTR_INVALID;
    *out_num_components = 0;
    return TF_DRACO_ERR_ARGUMENT;
  }
  *out_type = static_cast<int32_t>(att->attribute_type());
  *out_num_components = att->num_components();
  return TF_DRACO_OK;
}

// Reads an attribute as float, one value per point, in point order. |out| must
// hold num_points * num_components floats.
int32_t tf_draco_decoded_read_f32(const TfDracoDecoded *decoded,
                                  uint32_t unique_id, float *out,
                                  char *err, size_t err_len) {
  if (decoded == nullptr || out == nullptr) {
    return TF_DRACO_ERR_ARGUMENT;
  }
  const draco::PointAttribute *att =
      decoded->mesh->GetAttributeByUniqueId(unique_id);
  if (att == nullptr) {
    SetError(err, err_len, "no attribute with that unique id");
    return TF_DRACO_ERR_ARGUMENT;
  }
  const int nc = att->num_components();
  if (nc > 4) {
    SetError(err, err_len, "attribute has more than four components");
    return TF_DRACO_ERR_UNSUPPORTED;
  }
  const uint32_t num_points = decoded->mesh->num_points();
  float value[4];
  for (uint32_t p = 0; p < num_points; ++p) {
    if (!att->ConvertValue<float>(att->mapped_index(draco::PointIndex(p)), nc,
                                  value)) {
      SetError(err, err_len, "attribute value does not convert to float");
      return TF_DRACO_ERR_DECODE;
    }
    memcpy(&out[static_cast<size_t>(p) * nc], value, sizeof(float) * nc);
  }
  return TF_DRACO_OK;
}

// The same, as unsigned 32-bit integers. Joint indices arrive this way.
int32_t tf_draco_decoded_read_u32(const TfDracoDecoded *decoded,
                                  uint32_t unique_id, uint32_t *out,
                                  char *err, size_t err_len) {
  if (decoded == nullptr || out == nullptr) {
    return TF_DRACO_ERR_ARGUMENT;
  }
  const draco::PointAttribute *att =
      decoded->mesh->GetAttributeByUniqueId(unique_id);
  if (att == nullptr) {
    SetError(err, err_len, "no attribute with that unique id");
    return TF_DRACO_ERR_ARGUMENT;
  }
  const int nc = att->num_components();
  if (nc > 4) {
    SetError(err, err_len, "attribute has more than four components");
    return TF_DRACO_ERR_UNSUPPORTED;
  }
  const uint32_t num_points = decoded->mesh->num_points();
  uint32_t value[4];
  for (uint32_t p = 0; p < num_points; ++p) {
    if (!att->ConvertValue<uint32_t>(att->mapped_index(draco::PointIndex(p)),
                                     nc, value)) {
      SetError(err, err_len, "attribute value does not convert to uint32");
      return TF_DRACO_ERR_DECODE;
    }
    memcpy(&out[static_cast<size_t>(p) * nc], value, sizeof(uint32_t) * nc);
  }
  return TF_DRACO_OK;
}

}  // extern "C"
