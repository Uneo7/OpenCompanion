#ifndef CONSOLE_UTIL_H
#define CONSOLE_UTIL_H

#include <stdint.h>

int  cu_str(uint8_t *o, int n, int max, const char *s);
int  cu_int(uint8_t *o, int n, int max, long v);
int  cu_hex(uint8_t *o, int n, int max, uint32_t v);          /* 8 hex digits */
/* "<label 0><v[0]><label 1><v[1]>..." appended at n; returns the new length. lab holds the cnt labels separated
 * by '\0' ("PR:\0 KN:"; an empty label is "\0\0"): one string instead of a table of pointers. A label must not
 * start with a digit (it would extend the "\0" escape). */
int  cu_fields(uint8_t *o, int n, int max, const char *lab, const long *v, int cnt);
long cu_fx(float v, float scale);                              /* rounded fixed point */
int  cu_parse_int(const uint8_t *a, int len, long *v);         /* "[-]d..d" (1..5 digits), 0 if malformed */

#endif
