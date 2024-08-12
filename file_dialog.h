#pragma once
#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void *(*malloc_t)(uintptr_t);

typedef struct {
  const char *name;
  uintptr_t name_capacity;
  double last_modified_ms;
  uint8_t *contents;
  uintptr_t contents_len;
} File;

typedef struct {
  const char *mime;
  const char *const *extensions;
  uintptr_t extensions_len;
} Accept;

typedef struct {
  const char *description;
  const Accept *accept;
  uintptr_t accept_len;
} FileType;

File LoadFile(const char *accept, malloc_t memalloc);
bool SaveFile(const uint8_t *contents, uintptr_t contents_len,
              const char *suggested_name, const char *suggested_mime,
              const FileType *types, uintptr_t types_len);

#ifdef __cplusplus
}
#endif