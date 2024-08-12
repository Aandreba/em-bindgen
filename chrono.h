#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

int32_t OffsetFromUtcDateTime(double utc_millis);
int32_t OffsetFromLocalDateTime(int32_t year, uint32_t month, uint32_t day,
                                uint32_t hour, uint32_t minute,
                                uint32_t second);

#ifdef __cplusplus
}
#endif