/* Application image in U7001: header page at EE_HDR, body at EE_BODY (format: ../docs/LAYOUT.md). */
#ifndef BOOT_IMAGE_H
#define BOOT_IMAGE_H

#include <stdint.h>

#define HDR_MAGIC       0x3149434Fu /* "OCI1" read little-endian */
#define HDR_VERSION     1u
#define HDR_SIZE        48u
#define HDR_CRC_OFF     44u
/* Body opcodes: AIS magic and Jump_Close (SPRAA69D table 9) + one OpenCompanion opcode (never seen by the ROM). */
#define AIS_MAGIC       0x41504954u
#define OP_JUMP_CLOSE   0x58535906u
#define OP_OC_LZ4_LOAD  0x58535980u /* address, raw size, compressed size, LZ4 block padded to 4 bytes */

#define HF_TRIAL        (1u << 0)   /* enforce the trial-boot counter (the app confirms itself) */
#define HF_KNOWN        (HF_TRIAL)

typedef struct {
    uint32_t seq;
    uint32_t body_len;
    uint32_t body_crc;
    uint32_t entry;
    uint32_t flags;
} img_hdr_t;

int image_read_header(img_hdr_t *h);                     /* R_NONE or a rescue reason */
int image_load(const img_hdr_t *h);                      /* loads + checks CRC; R_NONE or a reason */

#endif
