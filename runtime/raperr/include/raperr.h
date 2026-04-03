#ifndef RAP_RAPERR
#define RAP_RAPERR

#include <string.h>

void runtime_error_description(const char *source, const char *path,
                               size_t position_start, size_t position_end,
                               const char *msg);

#endif // RAP_RAPERR
