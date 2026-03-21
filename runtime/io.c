#include "runtime_internal.h"
#include <ctype.h>

// Read a line from stdin into a malloc'd buffer. Returns NULL on EOF.
static char *read_line(void) {
  size_t cap = 64, len = 0;
  char *buf = malloc(cap);
  int ch;
  while ((ch = getchar()) != EOF && ch != '\n') {
    if (len + 1 >= cap) {
      cap *= 2;
      buf = realloc(buf, cap);
    }
    buf[len++] = (char)ch;
  }
  if (len == 0 && ch == EOF) {
    free(buf);
    return NULL;
  }
  buf[len] = '\0';
  return buf;
}

RAP_Object *RAP_input_text(void) {
  char *line = read_line();
  if (!line) return RAP_create_text_obj("");
  RAP_Object *result = RAP_create_text_obj(line);
  free(line);
  return result;
}

RAP_Object *RAP_input_value(void) {
  char *line = read_line();
  if (!line) return RAP_create_null_obj();

  // Skip leading whitespace
  char *p = line;
  while (*p && isspace((unsigned char)*p)) p++;

  if (*p == '\0') {
    free(line);
    return RAP_create_null_obj();
  }

  // Try to parse as integer
  char *end;
  long long int_val = strtoll(p, &end, 10);
  if (*end == '\0' || (*end && isspace((unsigned char)*end))) {
    // Check there's nothing else after the number
    while (*end && isspace((unsigned char)*end)) end++;
    if (*end == '\0') {
      free(line);
      return RAP_create_int_obj((int64_t)int_val);
    }
  }

  // Try to parse as float
  double float_val = strtod(p, &end);
  if (*end == '\0' || (*end && isspace((unsigned char)*end))) {
    while (*end && isspace((unsigned char)*end)) end++;
    if (*end == '\0') {
      free(line);
      return RAP_create_float_obj(float_val);
    }
  }

  // Otherwise return as text
  RAP_Object *result = RAP_create_text_obj(p);
  free(line);
  return result;
}
