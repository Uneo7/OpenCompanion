#include "boot.h"
#include "stream.h"
#include "ee.h"
#include "crc32.h"

void st_init(stream_t *s, uint32_t pos, uint32_t len)
{
    s->pos = pos;
    s->left = len;
    s->crc = CRC32_INIT;
    s->i = s->n = 0;
    s->err = R_NONE;
}

static int refill(stream_t *s)
{
    uint32_t k = s->left < ST_BUF ? s->left : ST_BUF;
    if (s->err) return -1;
    if (k == 0) { s->err = R_BAD_BODY; return -1; }   /* past the declared length */
    if (ee_read(s->pos, s->buf, k)) { s->err = R_EE_IO; return -1; }
    s->crc = crc32_update(s->crc, s->buf, k);
    s->pos += k;
    s->i = 0;
    s->n = k;
    return 0;
}

int st_getc(void *p)
{
    stream_t *s = (stream_t *)p;
    if (s->i == s->n && refill(s)) return -1;
    s->left--;
    return s->buf[s->i++];
}

int st_read(stream_t *s, volatile uint8_t *dst, uint32_t n)
{
    while (n--) {
        int c = st_getc(s);
        if (c < 0) return s->err;
        *dst++ = (uint8_t)c;
    }
    return R_NONE;
}

int st_word(stream_t *s, uint32_t *v)
{
    uint8_t b[4];
    int r = st_read(s, b, 4);
    *v = rd32(b);
    return r;
}
