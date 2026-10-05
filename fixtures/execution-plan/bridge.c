#include <stdint.h>

#if defined(_WIN32)
#define ALGORAM_EXPORT __declspec(dllexport)
#else
#define ALGORAM_EXPORT __attribute__((visibility("default")))
#endif

ALGORAM_EXPORT int32_t algoram_checked_double(int32_t value) {
    if (value < 0) {
        return -1;
    }
    return value * 2;
}
