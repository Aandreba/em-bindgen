#pragma once
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void *(malloc_t)(uintptr_t);

typedef struct {
  const char *key;
  const char *value;
} fetch_header_t;

typedef struct {
  uint64_t timeout;
  const fetch_header_t *headers;
  uintptr_t headers_len;
  const uint8_t *body;
  uintptr_t body_len;
} fetch_attrs_t;

typedef enum {
  Sent = 0,
  TimedOut = 1,
  Exception = 2,
  Ended = 3,
} fetch_status_t;

typedef void (*fetch_onresponse_t)(fetch_status_t, uint16_t,
                                   const fetch_header_t *, uintptr_t, void *,
                                   void *);

typedef uint8_t *(*fetch_onbytes_pre)(uintptr_t, void *);
typedef void (*fetch_onbytes_post)(fetch_status_t, uint8_t *, uintptr_t,
                                   void *);

void SendRequest(const char *method, const char *url, fetch_attrs_t attrs,
                 fetch_onresponse_t onresponse, void *onresponse_userdata);

void GetResponseBytes(void *handle, fetch_onbytes_pre onbytes_pre,
                      void *onbytes_pre_userdata,
                      fetch_onbytes_post onbytes_post,
                      void *onbytes_post_userdata);

void GetResponseChunks(void *handle, fetch_onbytes_pre onbytes_pre,
                       void *onbytes_pre_userdata,
                       fetch_onbytes_post onbytes_post,
                       void *onbytes_post_userdata);

#ifdef __cplusplus
}
#endif