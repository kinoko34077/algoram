#include <stdint.h>

#if defined(_WIN32)
#define ALG_EXPORT __declspec(dllexport)
#else
#define ALG_EXPORT __attribute__((visibility("default")))
#endif

ALG_EXPORT int32_t algoram_double(int32_t value) {
    return value * 2;
}
