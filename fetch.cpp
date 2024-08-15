#include "fetch.h"
#include <emscripten.h>
#include <emscripten/val.h>

using namespace emscripten;

#ifdef __cplusplus
extern "C" {
#endif

void SendRequest(const char *method, const char *url, fetch_attrs_t attrs,
                 fetch_onresponse_t onresponse, void *onresponse_userdata) {
  val headers = val::object();
  for (uintptr_t i = 0; i < attrs.headers_len; i++) {
    const fetch_header_t *header = &attrs.headers[i];
    headers.set(val::u8string(header->key), val::u8string(header->value));
  }

  EM_ASM(
      {
        const onresponse = Module.cwrap("_INTERNAL_ON_RESPONSE", "void", [
          "number", "number", "number", "number", "number", "number", "number"
        ]);

        fetch(UTF8ToString($1), {
          method : UTF8ToString($0),
          headers : Emval.toValue($4),
          body : ($2 == 0) ? undefined : HEAPU8.slice($2, $2 + $3),
          signal : ($5 == 0) ? undefined : AbortSignal.timeout($5),
        })
            .then(function(resp) {
              console.log(resp);
              const headers = resp.headers.entries();
              const rawHeaders =
                  ("flatMap" in headers)
                      ? headers.flatMap(function(kv) { return kv.values(); })
                            .map(function(str) {
                              const ptr = Module._malloc(4 * str.length + 1);
                              stringToUTF8(str, ptr, 4 * str.length + 1);
                              return ptr;
                            })
                            .toArray()
                      : [... headers]
                            .flatMap(function(kv) { return kv; })
                            .map(function(str) {
                              const ptr = Module._malloc(4 * str.length + 1);
                              stringToUTF8(str, ptr, 4 * str.length + 1);
                              return ptr;
                            });

              try {
                const headers_ptr = Module._malloc(rawHeaders.length << 2);
                try {
                  HEAPU32
                      .subarray(headers_ptr >> 2,
                                (headers_ptr >> 2) + rawHeaders.length)
                      .set(rawHeaders);
                  (onresponse)($6, 0, resp.status, headers_ptr,
                               rawHeaders.length >> 1, Emval.toHandle(resp),
                               $7);
                } finally {
                  Module._free(headers_ptr);
                }
              } finally {
                for (entry of rawHeaders)
                  Module._free(entry);
              }
            })
            .catch(function(e) {
              const isTimeout = e.name == "TimeoutError";
              if (!isTimeout)
                console.error(e);
              (onresponse)($6, isTimeout ? 1 : 2, 0, 0, 0, 0, $7);
            });
      },
      method, url, attrs.body, attrs.body_len, headers.as_handle(),
      (double)attrs.timeout, onresponse, onresponse_userdata);
}

void GetResponseBytes(void *handle, fetch_onbytes_pre onbytes_pre,
                      void *onbytes_pre_userdata,
                      fetch_onbytes_post onbytes_post,
                      void *onbytes_post_userdata) {
  EM_ASM(
      {
        const onbytes_post =
            Module.cwrap("_INTERNAL_ON_BYTES_POST", "number",
                         [ "number", "number", "number", "number", "number" ]);

        Emval.toValue($0)
            .arrayBuffer()
            .then(function(buffer) {
              const bytes = new Uint8Array(buffer);
              const dst = Module.ccall("_INTERNAL_ON_BYTES_PRE", "number",
                                       [ "number", "number", "number" ],
                                       [ $1, bytes.byteLength, $2 ]);
              HEAPU8.subarray(dst, dst + bytes.byteLength).set(bytes);
              onbytes_post($3, 0, dst, bytes.byteLength, $4);
            })
            .catch(function(e) {
              const isTimeout = e.name == "TimeoutError";
              if (!isTimeout)
                console.error(e);
              onbytes_post($3, isTimeout ? 1 : 2, 0, 0, $4);
            });
      },
      (EM_VAL)handle, onbytes_pre, onbytes_pre_userdata, onbytes_post,
      onbytes_post_userdata);
}

void GetResponseChunks(void *handle, fetch_onbytes_pre onbytes_pre,
                       void *onbytes_pre_userdata,
                       fetch_onbytes_post onbytes_post,
                       void *onbytes_post_userdata) {
  EM_ASM(
      {
        const response = Emval.toValue($0);
        const onbytes_post =
            Module.cwrap("_INTERNAL_ON_BYTES_POST", "number",
                         [ "number", "number", "number", "number", "number" ]);

        (async function() {
          const reader = response.body.getReader();
          try {
            while (true) {
              const {done, value : bytes} = await reader.read();
              if (done) {
                onbytes_post($3, 3, 0, 0, $4);
                break;
              }

              const dst = Module.ccall("_INTERNAL_ON_BYTES_PRE", "number",
                                       [ "number", "number", "number" ],
                                       [ $1, bytes.byteLength, $2 ]);
              HEAPU8.subarray(dst, dst + bytes.byteLength).set(bytes);
              onbytes_post($3, 0, dst, bytes.byteLength, $4);
            }
          } catch (e) {
            reader.cancel();
            const isTimeout = e.name == "TimeoutError";
            if (!isTimeout)
              console.error(e);
            onbytes_post($3, isTimeout ? 1 : 2, 0, 0, $4);
          }
        })();
      },
      (EM_VAL)handle, onbytes_pre, onbytes_pre_userdata, onbytes_post,
      onbytes_post_userdata);
}

EMSCRIPTEN_KEEPALIVE
void _INTERNAL_ON_RESPONSE(fetch_onresponse_t cb, fetch_status_t status,
                           uint16_t status_code, const fetch_header_t *headers,
                           uintptr_t headers_len, void *handle,
                           void *user_data) {
  (cb)(status, status_code, headers, headers_len, handle, user_data);
}

EMSCRIPTEN_KEEPALIVE
uint8_t *_INTERNAL_ON_BYTES_PRE(fetch_onbytes_pre cb, uintptr_t len,
                                void *user_data) {
  return (cb)(len, user_data);
}

EMSCRIPTEN_KEEPALIVE
void _INTERNAL_ON_BYTES_POST(fetch_onbytes_post cb, fetch_status_t status,
                             uint8_t *ptr, uintptr_t len, void *user_data) {
  (cb)(status, ptr, len, user_data);
}

#ifdef __cplusplus
}
#endif