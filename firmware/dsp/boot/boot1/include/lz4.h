/* LZ4 block decoder (format: lz4/doc/lz4_Block_format.md), streaming input, output straight into RAM.
 * Portable C: also compiled on the host by test/host_image.c (test/run_host_tests.sh). */
#ifndef BOOT_LZ4_H
#define BOOT_LZ4_H

#include <stdint.h>

typedef int (*lz4_getc_fn)(void *ctx);  /* next input byte 0..255, or -1 */

/* Decodes exactly raw bytes into dst. Returns 0, or -1 on malformed input (truncated stream, match offset
 * before dst, output overrun). Never writes outside [dst, dst + raw). */
int lz4_decode(volatile uint8_t *dst, uint32_t raw, lz4_getc_fn in, void *ctx);

#endif
