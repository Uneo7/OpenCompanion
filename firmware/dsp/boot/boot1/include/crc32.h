/* CRC-32 IEEE 802.3 (same as zlib.crc32 / crc32fast). Usage: c = CRC32_INIT; c = crc32_update(c, p, n)...;
 * result = ~c. */
#ifndef BOOT_CRC32_H
#define BOOT_CRC32_H

#include <stdint.h>

#define CRC32_INIT 0xFFFFFFFFu

uint32_t crc32_update(uint32_t c, const volatile uint8_t *p, uint32_t n);
uint32_t crc32(const volatile uint8_t *p, uint32_t n);

#endif
