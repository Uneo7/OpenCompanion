/* CRC-32 (IEEE 802.3 / zlib, reflected 0xEDB88320), bitwise: no table, the image size matters more than
 * speed here (profiles: 616 bytes per save or load; boot records: 12 / 44 bytes). Same function as
 * boot1/src/crc32.c and the host tools. */
#include "crc32.h"

uint32_t oc_crc32_update(uint32_t c, const uint8_t *p, uint32_t n)
{
    int k;
    while (n--) {
        c ^= *p++;
        for (k = 0; k < 8; k++) c = (c >> 1) ^ (0xEDB88320u & (0u - (c & 1u)));
    }
    return c;
}

uint32_t oc_crc32(const uint8_t *p, uint32_t n) { return ~oc_crc32_update(0xFFFFFFFFu, p, n); }

uint32_t oc_get32(const uint8_t *p)
{
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

void oc_put32(uint8_t *p, uint32_t v)
{
    p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); p[2] = (uint8_t)(v >> 16); p[3] = (uint8_t)(v >> 24);
}
