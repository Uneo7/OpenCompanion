#ifndef RESTORE_HEADER_H
#define RESTORE_HEADER_H

#include <stdint.h>

#define CMD_RESTORE_HEADER  0x20

/* Builds the reply in `out` and returns its length (see restore_header.c). */
uint8_t restore_header(__xdata uint8_t *out);

#endif /* RESTORE_HEADER_H */
