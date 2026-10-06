// ABOUTME: Sends grid encodes at the edges of the native arithmetic domain to Draco.
// ABOUTME: Run it under UBSan to test the Rust grid-domain guard against the native code.
// Standalone: link against a static Draco build. The Rust guard in
// validate_grid_domain refuses every case marked "refused" below, so none of
// them reaches the native code through the crate. This probe calls the C
// entry point directly. A sanitizer report in a refused case shows that the
// guard is necessary there. A clean, exact round trip in an accepted case
// shows that the guard is not stricter than it must be at that edge.
#include <sys/wait.h>
#include <unistd.h>

#include <cstdio>

#include "../csrc/tileforge_draco.cc"

namespace {

struct Case {
  const char *name;
  bool refused;  // the verdict of the Rust guard for this input
  float spacing;
  float lo;  // x of the first vertex
  float hi;  // x of the second vertex
};

// The values are grid indices times the spacing. Every bound is exact in
// binary32. Near 2^30, binary32 holds multiples of 64 below 2^30 and of 128
// above it, so a span of exactly 2^30 values needs one small odd bound.
const Case kCases[] = {
    // The widest span the guard accepts: -63 to 2^30 - 64 is 2^30 values.
    {"span-2^30-values", false, 1.0f, -63.0f, 1073741760.0f},
    // The guard refuses one more value. Draco counts the values in binary32,
    // which rounds 2^30 + 1 down to 2^30, so Draco still uses 30 bits.
    {"span-2^30+1-values", true, 1.0f, -64.0f, 1073741760.0f},
    // 2^30 + 129 values round to 2^30 + 128 and need 31 bits.
    {"span-2^30+129-values", true, 1.0f, -128.0f, 1073741824.0f},
    // The lowest index the guard accepts, with a short span.
    {"index-min-i32", false, 1.0f, -2147483648.0f, -2147483648.0f + 1024},
    // The highest binary32 index below 2^31, with a short span.
    {"index-max-below-2^31", false, 1.0f, 2147483520.0f - 1024,
     2147483520.0f},
    // 2^31 does not fit in a signed 32-bit index.
    {"index-2^31", true, 1.0f, 2147483648.0f - 1024, 2147483648.0f},
    // The four refused cases in tests/grid_domain.rs.
    {"grid_domain-positive", true, 1.0f / 4096, 1000000.0f, 1000001.0f},
    {"grid_domain-negative", true, 1.0f / 4096, -1000001.0f, -1000000.0f},
    {"grid_domain-span", true, 1.0f, -1000000000.0f, 1000000000.0f},
    {"grid_domain-float-max", true, 1.0f, 0.0f, 3.4028235e38f},
};

// Encodes a triangle that spans [lo, hi] on x and decodes it. Returns 0 when
// every decoded position equals an input position.
int RoundTrip(const Case &c) {
  const float positions[] = {c.lo, 0, 0, c.hi, 0, 0, c.lo, 1, 0};
  const uint32_t indices[] = {0, 1, 2};
  TfDracoAttribute att{TF_DRACO_ATTR_POSITION, 3, 0, positions, nullptr, 0};
  TfDracoMesh mesh{&att, 1, indices, 3, 1};
  TfDracoEncodeOptions opts{c.spacing, 0, 0};
  TfDracoBuffer out{};
  uint32_t id = 0;
  char err[512] = {};
  int code = tf_draco_encode(&mesh, &opts, &out, &id, err, sizeof(err));
  if (code != 0) {
    std::printf("  encode code=%d: %s\n", code, err);
    return 1;
  }
  TfDracoDecoded *decoded = nullptr;
  code = tf_draco_decode(out.data, out.len, &decoded, err, sizeof(err));
  tf_draco_buffer_free(&out);
  if (code != 0) {
    std::printf("  decode code=%d: %s\n", code, err);
    return 1;
  }
  const uint32_t n = tf_draco_decoded_num_points(decoded);
  float values[3 * 3] = {};
  int result = 0;
  if (n != 3 || tf_draco_decoded_read_f32(decoded, id, values, err,
                                          sizeof(err)) != 0) {
    std::printf("  decoded %u points\n", n);
    result = 1;
  } else {
    for (uint32_t p = 0; p < n; ++p) {
      bool found = false;
      for (int v = 0; v < 3; ++v) {
        found |= values[3 * p] == positions[3 * v] &&
                 values[3 * p + 1] == positions[3 * v + 1] &&
                 values[3 * p + 2] == positions[3 * v + 2];
      }
      if (!found) {
        std::printf("  decoded point %u (%.9g, %.9g, %.9g) is not an input\n",
                    p, values[3 * p], values[3 * p + 1], values[3 * p + 2]);
        result = 1;
      }
    }
  }
  tf_draco_decoded_free(decoded);
  return result;
}

}  // namespace

int main() {
  int unexpected = 0;
  for (const Case &c : kCases) {
    std::printf("CASE %s guard=%s\n", c.name,
                c.refused ? "refused" : "accepted");
    std::fflush(nullptr);
    const pid_t pid = fork();
    if (pid == 0) _exit(RoundTrip(c));
    int status = 0;
    waitpid(pid, &status, 0);
    const bool exact = WIFEXITED(status) && WEXITSTATUS(status) == 0;
    std::printf("RESULT %s round_trip=%s status=%d\n", c.name,
                exact ? "exact" : "failed", status);
    if (!c.refused && !exact) ++unexpected;
  }
  std::printf("accepted cases that failed=%d\n", unexpected);
  return unexpected == 0 ? 0 : 1;
}
