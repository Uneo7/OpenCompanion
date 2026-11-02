/* Small replacements for the RTS memset (~290 bytes of unrolled code) and memcpy (memcpy62, 512 bytes). Only
 * control code uses them (profile, console and I2C frame buffers, at most a 616-byte profile): a byte loop is
 * fast enough, and the image must fit the EEPROM body (boot/docs/LAYOUT.md). The audio path never calls them. */
#include <string.h>

void *memset(void *d, int c, size_t n)
{
    unsigned char *p = (unsigned char *)d;
    while (n--) *p++ = (unsigned char)c;
    return d;
}

void *memcpy(void *d, const void *s, size_t n)
{
    unsigned char *p = (unsigned char *)d;
    const unsigned char *q = (const unsigned char *)s;
    while (n--) *p++ = *q++;
    return d;
}
