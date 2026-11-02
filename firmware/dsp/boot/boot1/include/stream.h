/* Buffered reader over the image body in the EEPROM. Every byte delivered is added to the running CRC-32, so
 * the CRC covers exactly the bytes that were interpreted. Reading past body_len is an error. */
#ifndef BOOT_STREAM_H
#define BOOT_STREAM_H

#include <stdint.h>

#define ST_BUF 128u

typedef struct {
    uint32_t pos;           /* EEPROM address of the next byte to fetch into buf */
    uint32_t left;          /* body bytes not yet delivered */
    uint32_t crc;           /* running CRC-32 (not inverted) */
    uint32_t i, n;          /* buf read index / valid bytes */
    int err;                /* R_* reason of the first error, sticky */
    uint8_t buf[ST_BUF];
} stream_t;

void st_init(stream_t *s, uint32_t pos, uint32_t len);
int  st_getc(void *s);                                  /* 0..255, or -1 (s->err set) */
int  st_read(stream_t *s, volatile uint8_t *dst, uint32_t n);   /* R_NONE or reason */
int  st_word(stream_t *s, uint32_t *v);

#endif
