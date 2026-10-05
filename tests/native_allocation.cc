// ABOUTME: Injects allocation failure through native encode and decode calls.
// ABOUTME: Checks that every failure returns an error code and leaves outputs clear.
// Standalone fault injection: link against the same static Draco build.
//
// The probe sweeps every operator new call that one encode or one decode
// makes. In single mode, only the chosen allocation fails. In persistent
// mode, the chosen allocation and every later allocation fail, which models
// memory exhaustion. Each attempt runs in a child process, so an attempt that
// terminates the process is reported with its allocation number and the sweep
// continues.
#include <sys/wait.h>
#include <unistd.h>

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <new>

static long fail_after = -1;
static bool persistent = false;
static bool injected = false;

static bool InjectFailure() {
  if (fail_after == 0) {
    if (!persistent) fail_after = -1;
    injected = true;
    return true;
  }
  if (fail_after > 0) --fail_after;
  return false;
}

void *operator new(std::size_t n) {
  if (InjectFailure()) throw std::bad_alloc();
  if (void *p = std::malloc(n ? n : 1)) return p;
  throw std::bad_alloc();
}
void *operator new[](std::size_t n) { return ::operator new(n); }
void *operator new(std::size_t n, std::align_val_t a) {
  if (InjectFailure()) throw std::bad_alloc();
  const std::size_t align = static_cast<std::size_t>(a);
  const std::size_t size = ((n ? n : 1) + align - 1) / align * align;
  if (void *p = std::aligned_alloc(align, size)) return p;
  throw std::bad_alloc();
}
void *operator new[](std::size_t n, std::align_val_t a) {
  return ::operator new(n, a);
}
void operator delete(void *p) noexcept { std::free(p); }
void operator delete[](void *p) noexcept { std::free(p); }
void operator delete(void *p, std::size_t) noexcept { std::free(p); }
void operator delete[](void *p, std::size_t) noexcept { std::free(p); }
void operator delete(void *p, std::align_val_t) noexcept { std::free(p); }
void operator delete[](void *p, std::align_val_t) noexcept { std::free(p); }
void operator delete(void *p, std::size_t, std::align_val_t) noexcept {
  std::free(p);
}
void operator delete[](void *p, std::size_t, std::align_val_t) noexcept {
  std::free(p);
}

#include "../csrc/tileforge_draco.cc"

namespace {

// Child exit codes. Any other status, including a signal, is a defect.
constexpr int kCaught = 10;     // the injected failure returned an error
constexpr int kCompleted = 11;  // no failure was injected and the call passed
// The library absorbed the injected failure and the encode returned the same
// bytes as the control encode. std::stable_sort does this: it asks for a
// temporary buffer with nothrow new and sorts in place when it gets none.
constexpr int kRecovered = 12;

// A 3 by 3 vertex patch on the 1/256 grid, with texture coordinates in the
// unit square. It is large enough for the edgebreaker traversal.
const float kPatchPositions[] = {
    0, 0, 0, 0.5f, 0, 0, 1, 0, 0,
    0, 0.5f, 0, 0.5f, 0.5f, 0.25f, 1, 0.5f, 0,
    0, 1, 0, 0.5f, 1, 0, 1, 1, 0};
const float kPatchUvs[] = {0,    0,    0.5f, 0,    1,    0,
                           0,    0.5f, 0.5f, 0.5f, 1,    0.5f,
                           0,    1,    0.5f, 1,    1,    1};
const uint32_t kPatchIndices[] = {0, 1, 4, 0, 4, 3, 1, 2, 5, 1, 5, 4,
                                  3, 4, 7, 3, 7, 6, 4, 5, 8, 4, 8, 7};
const float kTrianglePositions[] = {0, 0, 0, 1, 0, 0, 0, 1, 0};
const uint32_t kTriangleIndices[] = {0, 1, 2};
const float kUnitOrigin[] = {0, 0};

struct Case {
  const char *name;
  TfDracoAttribute attributes[2];
  uint32_t num_attributes;
  const uint32_t *indices;
  uint32_t num_vertices;
  uint32_t num_faces;
  TfDracoEncodeOptions options;
};

// The bit-count triangle, and the attribute shapes the two callers send.
const Case kCases[] = {
    {"bits-triangle",
     {{TF_DRACO_ATTR_POSITION, 3, 14, kTrianglePositions, nullptr, 0}},
     1, kTriangleIndices, 3, 1, {0, 0, 0}},
    {"grid-unit-uv",
     {{TF_DRACO_ATTR_POSITION, 3, 0, kPatchPositions, nullptr, 0},
      {TF_DRACO_ATTR_TEX_COORD, 2, 12, kPatchUvs, kUnitOrigin, 1}},
     2, kPatchIndices, 9, 8, {1.0f / 256, 0, 0}},
    {"bits-fitted-uv",
     {{TF_DRACO_ATTR_POSITION, 3, 14, kPatchPositions, nullptr, 0},
      {TF_DRACO_ATTR_TEX_COORD, 2, 10, kPatchUvs, nullptr, 0}},
     2, kPatchIndices, 9, 8, {0, 0, 0}},
    {"lossless-unit-uv",
     {{TF_DRACO_ATTR_POSITION, 3, 0, kPatchPositions, nullptr, 0},
      {TF_DRACO_ATTR_TEX_COORD, 2, 10, kPatchUvs, kUnitOrigin, 1}},
     2, kPatchIndices, 9, 8, {0, 0, 1}},
};

TfDracoMesh MeshOf(const Case &c) {
  return {c.attributes, c.num_attributes, c.indices, c.num_vertices,
          c.num_faces};
}

// Runs one call with allocation |n| failing. Returns a child exit code.
int Attempt(const Case &c, const TfDracoBuffer &valid, bool decoding,
            long n) {
  const TfDracoMesh mesh = MeshOf(c);
  TfDracoBuffer out{};
  TfDracoDecoded *decoded = nullptr;
  uint32_t ids[2] = {};
  char err[512];
  fail_after = n;
  injected = false;
  int code;
  try {
    code = decoding
               ? tf_draco_decode(valid.data, valid.len, &decoded, err,
                                 sizeof(err))
               : tf_draco_encode(&mesh, &c.options, &out, ids, err,
                                 sizeof(err));
  } catch (...) {
    fail_after = -1;
    return 2;
  }
  fail_after = -1;
  if (injected && code == 0 && !decoding && out.len == valid.len &&
      std::memcmp(out.data, valid.data, out.len) == 0) {
    return kRecovered;
  }
  if (injected) {
    return code != 0 && out.data == nullptr && decoded == nullptr ? kCaught
                                                                  : 3;
  }
  if (code != 0) return 4;
  tf_draco_buffer_free(&out);
  tf_draco_decoded_free(decoded);
  return kCompleted;
}

// Sweeps allocation numbers until a call completes without an injected
// failure. Returns the number of defective attempts.
int Sweep(const Case &c, const TfDracoBuffer &valid, bool decoding) {
  long caught = 0;
  long recovered = 0;
  int defects = 0;
  bool completed = false;
  for (long n = 0; n < 100000 && !completed; ++n) {
    std::fflush(nullptr);
    const pid_t pid = fork();
    if (pid < 0) {
      std::perror("fork");
      return defects + 1;
    }
    if (pid == 0) _exit(Attempt(c, valid, decoding, n));
    int status = 0;
    if (waitpid(pid, &status, 0) != pid) {
      std::perror("waitpid");
      return defects + 1;
    }
    if (WIFEXITED(status) && WEXITSTATUS(status) == kCaught) {
      ++caught;
    } else if (WIFEXITED(status) && WEXITSTATUS(status) == kRecovered) {
      ++recovered;
    } else if (WIFEXITED(status) && WEXITSTATUS(status) == kCompleted) {
      completed = true;
    } else {
      ++defects;
      if (WIFSIGNALED(status)) {
        std::printf("DEFECT mode=%s op=%s case=%s allocation=%ld signal=%d\n",
                    persistent ? "persistent" : "single",
                    decoding ? "decode" : "encode", c.name, n,
                    WTERMSIG(status));
      } else {
        std::printf("DEFECT mode=%s op=%s case=%s allocation=%ld exit=%d\n",
                    persistent ? "persistent" : "single",
                    decoding ? "decode" : "encode", c.name, n,
                    WEXITSTATUS(status));
      }
    }
  }
  std::printf(
      "mode=%s op=%s case=%s caught=%ld recovered=%ld defects=%d "
      "completed=%d\n",
      persistent ? "persistent" : "single", decoding ? "decode" : "encode",
      c.name, caught, recovered, defects, completed);
  if (!completed || caught == 0) ++defects;
  return defects;
}

}  // namespace

int main() {
  int defects = 0;
  for (const Case &c : kCases) {
    const TfDracoMesh mesh = MeshOf(c);
    TfDracoBuffer valid{};
    char err[512];
    if (tf_draco_encode(&mesh, &c.options, &valid, nullptr, err,
                        sizeof(err)) != 0) {
      std::printf("control encode failed: case=%s: %s\n", c.name, err);
      return 1;
    }
    for (bool mode : {false, true}) {
      persistent = mode;
      for (bool decoding : {false, true}) {
        defects += Sweep(c, valid, decoding);
      }
    }
    persistent = false;
    tf_draco_buffer_free(&valid);
  }
  std::printf("total defects=%d\n", defects);
  return defects == 0 ? 0 : 1;
}
