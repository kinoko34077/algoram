#if defined(_WIN32)
#define ALGORAM_EXPORT __declspec(dllexport)
#else
#define ALGORAM_EXPORT __attribute__((visibility("default")))
#endif

ALGORAM_EXPORT int algoram_double(int value) {
    return value * 2;
}
