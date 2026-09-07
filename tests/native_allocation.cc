// Standalone fault injection: link against the same static Draco build.
#include <cstdlib>
#include <cstdio>
#include <new>
static long fail_after = -1;
static bool injected = false;
void *operator new(std::size_t n) {
  if (fail_after == 0) { fail_after = -1; injected = true; throw std::bad_alloc(); }
  if (fail_after > 0) --fail_after;
  if (void *p = std::malloc(n ? n : 1)) return p;
  throw std::bad_alloc();
}
void *operator new[](std::size_t n) { return ::operator new(n); }
void operator delete(void *p) noexcept { std::free(p); }
void operator delete[](void *p) noexcept { std::free(p); }
void operator delete(void *p, std::size_t) noexcept { std::free(p); }
void operator delete[](void *p, std::size_t) noexcept { std::free(p); }
#include "../csrc/tileforge_draco.cc"
int main() {
  const float positions[] = {0,0,0, 1,0,0, 0,1,0};
  const uint32_t indices[] = {0,1,2};
  TfDracoAttribute att{TF_DRACO_ATTR_POSITION, 3, 14, positions, nullptr, 0};
  TfDracoMesh mesh{&att, 1, indices, 3, 1};
  TfDracoEncodeOptions opts{0, 0};
  char err[512];
  TfDracoBuffer valid{};
  if (tf_draco_encode(&mesh, &opts, &valid, nullptr, err, sizeof(err))) return 1;
  for (bool decoding : {false, true}) {
    long failures = 0;
    bool completed = false;
    for (long n = 0; n < 10000; ++n) {
      TfDracoBuffer out{};
      TfDracoDecoded *decoded = nullptr;
      std::fprintf(stderr, "probe decode=%d allocation=%ld\n", decoding, n);
      fail_after = n; injected = false;
      int code;
      try {
        code = decoding ? tf_draco_decode(valid.data, valid.len, &decoded, err, sizeof(err))
          : tf_draco_encode(&mesh, &opts, &out, nullptr, err, sizeof(err));
      } catch (...) {
        fail_after = -1;
        std::fprintf(stderr, "escaped exception: decode=%d allocation=%ld\n", decoding, n);
        return 2;
      }
      fail_after = -1;
      if (injected) {
        ++failures;
        if (code == 0 || out.data || decoded) return 3;
      } else {
        if (code != 0) return 4;
        completed = true;
      }
      tf_draco_buffer_free(&out);
      tf_draco_decoded_free(decoded);
      if (completed) break;
    }
    std::printf("decode=%d caught failures=%ld completed=%d\n", decoding, failures, completed);
    if (!completed || !failures) return 5;
  }
  tf_draco_buffer_free(&valid);
}
