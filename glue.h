#include <emscripten/val.h>
#include <stdint.h>

typedef struct {
  _Alignas(alignof(emscripten::val)) uint8_t inner[sizeof(emscripten::val)];
} js_value_t;
