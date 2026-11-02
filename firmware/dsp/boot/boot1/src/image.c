/* Image header check and body load (format: ../docs/LAYOUT.md).
 *
 * Body: AIS magic, then LZ4 sections (address, raw size, compressed size, LZ4 block padded to 4 bytes), then
 * Jump_Close (entry). The host tool stores every ELF segment and every zero fill as an LZ4 section, so boot1
 * needs no other opcode. The body is read from the EEPROM in ONE pass: data is decoded straight to its
 * destination and the CRC-32 covers every body byte as read. Nothing is executed unless the whole body was
 * consumed, its length matches the header, the CRC matches and the Jump_Close target matches the header
 * entry. Every destination range is checked against [APP_RAM_LO, APP_RAM_HI) BEFORE anything is written
 * there, and the decoder never writes outside it, so a corrupted body can only scribble over application
 * RAM, never over boot1, its stack or the trampoline. */
#include "boot.h"
#include "ee.h"
#include "crc32.h"
#include "image.h"
#include "stream.h"
#include "lz4.h"
#include "layout.h"

static int in_app_ram(uint32_t a, uint32_t n)
{
    return a >= APP_RAM_LO && a <= APP_RAM_HI && n <= APP_RAM_HI - a;
}

int image_read_header(img_hdr_t *h)
{
    uint8_t b[HDR_SIZE];
    if (ee_read(EE_HDR, b, HDR_SIZE)) return R_EE_IO;
    if (rd32(b) != HDR_MAGIC) return R_NO_HEADER;
    if (crc32(b, HDR_CRC_OFF) != rd32(b + HDR_CRC_OFF)) return R_BAD_HEADER;
    if (rd32(b + 4) != (HDR_VERSION | (HDR_SIZE << 16))) return R_BAD_HEADER;
    h->seq = rd32(b + 8);
    h->body_len = rd32(b + 12);
    h->body_crc = rd32(b + 16);
    h->entry = rd32(b + 20);
    h->flags = rd32(b + 24);
    if (h->body_len < 12u || h->body_len > EE_BODY_MAX || (h->body_len & 3u)) return R_BAD_HEADER;
    if ((h->flags & ~HF_KNOWN) || !in_app_ram(h->entry, 4)) return R_BAD_HEADER;
    return R_NONE;
}

static int section_lz4(stream_t *s)
{
    uint32_t a, raw, comp, start;
    uint8_t pad[4];
    int r;
    if ((r = st_word(s, &a)) || (r = st_word(s, &raw)) || (r = st_word(s, &comp))) return r;
    if (!in_app_ram(a, raw) || comp > s->left) return R_BAD_BODY;
    start = s->left;
    if (lz4_decode((volatile uint8_t *)a, raw, st_getc, s)) return s->err ? s->err : R_BAD_BODY;
    if (start - s->left != comp) return R_BAD_BODY;
    return st_read(s, pad, (4u - (comp & 3u)) & 3u);
}

int image_load(const img_hdr_t *h)
{
    static stream_t s;
    uint32_t op;
    int r;
    st_init(&s, EE_BODY, h->body_len);
    if ((r = st_word(&s, &op))) return r;
    if (op != AIS_MAGIC) return R_BAD_BODY;
    for (;;) {
        if ((r = st_word(&s, &op))) return r;
        if (op == OP_JUMP_CLOSE) break;
        if (op != OP_OC_LZ4_LOAD) return R_BAD_BODY;
        if ((r = section_lz4(&s))) return r;
    }
    if ((r = st_word(&s, &op))) return r;           /* Jump_Close target */
    if (s.left != 0 || op != h->entry) return R_BAD_BODY;
    if (~s.crc != h->body_crc) return R_BAD_CRC;
    return R_NONE;
}
