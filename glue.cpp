#include "glue.h"
#include <emscripten/val.h>

using namespace emscripten;

#ifdef __cplusplus
extern "C" {
#endif

GLUE_EM_VAL glue_get_global(const char *name) {}

void glue_destroy_value(GLUE_EM_VAL obj) {
  auto _ = val::take_ownership((EM_VAL)obj);
}

#ifdef __cplusplus
}
#endif