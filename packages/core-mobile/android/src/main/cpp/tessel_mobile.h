#pragma once

// The C ABI of tessel-mobile (packages/core-mobile/rust, story 02-13). Every call is synchronous; the
// buffers it fills are freed with tessel_buf_free.

#include <cstddef>
#include <cstdint>

extern "C" {

struct TesselBuf {
  uint8_t* ptr;
  size_t len;
  size_t cap;
};

// The platform's seal (unseal == 0) or unseal (unseal == 1) of the local database's key:
// writes into `out` (at most `outCap` bytes) and returns the length written, or -1.
typedef int64_t (*TesselSealFn)(int32_t unseal, const uint8_t* in, size_t len, uint8_t* out, size_t outCap);

// Where the database lives, and how its key is sealed. Once, before any call; 0 on success.
int32_t tessel_configure(const uint8_t* dir, size_t len, TesselSealFn seal);

// Runs `method` with its MessagePack arguments. 0: `out` holds the MessagePack answer;
// otherwise `out` holds "<code>\n<message>", the TesselError to throw.
int32_t tessel_call(const uint8_t* method, size_t methodLen, const uint8_t* args, size_t argsLen, TesselBuf* out);

void tessel_buf_free(TesselBuf buf);
}
