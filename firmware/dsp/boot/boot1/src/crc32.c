/* CRC-32 IEEE, nibble table (64 bytes), same code as the RAM loader. */
#include "crc32.h"

static const uint32_t t[16] = {
    0x00000000, 0x1DB71064, 0x3B6E20C8, 0x26D930AC, 0x76DC4190, 0x6B6B51F4, 0x4DB26158, 0x5005713C,
    0xEDB88320, 0xF00F9344, 0xD6D6A3E8, 0xCB61B38C, 0x9B64C2B0, 0x86D3D2D4, 0xA00AE278, 0xBDBDF21C };

uint32_t crc32_update(uint32_t c, const volatile uint8_t *p, uint32_t n)
{
    while (n--) {
        c ^= *p++;
        c = (c >> 4) ^ t[c & 15];
        c = (c >> 4) ^ t[c & 15];
    }
    return c;
}

uint32_t crc32(const volatile uint8_t *p, uint32_t n)
{
    return ~crc32_update(CRC32_INIT, p, n);
}
