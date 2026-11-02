#ifndef OC_CRC32_H
#define OC_CRC32_H

#include <stdint.h>

uint32_t oc_crc32(const uint8_t *p, uint32_t n);
uint32_t oc_crc32_update(uint32_t c, const uint8_t *p, uint32_t n);   /* raw register: start 0xFFFFFFFF, final ~ */
uint32_t oc_get32(const uint8_t *p);
void     oc_put32(uint8_t *p, uint32_t v);

#endif
