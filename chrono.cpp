#include "chrono.h"
#include <emscripten.h>

#ifdef __cplusplus
extern "C" {
#endif

int32_t OffsetFromUtcDateTime(double utc_millis) {
  return EM_ASM_INT({ return new Date($0).getTimezoneOffset(); }, utc_millis);
}

int32_t OffsetFromLocalDateTime(int32_t year, uint32_t month, uint32_t day,
                                uint32_t hour, uint32_t minute,
                                uint32_t second) {
  return EM_ASM_INT(
      { return new Date($0, $1, $2, $3, $4, $5).getTimezoneOffset(); }, year,
      month, day, hour, minute, second);
}

#ifdef __cplusplus
}
#endif