/* Trial-boot counter (only for images whose header has HF_TRIAL). Catches an image that is intact (CRC OK)
 * but never reaches the point where it confirms itself: after BOOT_TRIALS unconfirmed starts, boot1 stays in
 * rescue. The counter lives in the EEPROM (EE_STATE), so it works without RAM retention across resets.
 *
 * Cost: one page write per start of an unconfirmed image (never for a confirmed one). A power loss during
 * that write leaves an invalid state page, read as "0 attempts": at worst one extra trial. */
#include "boot.h"
#include "ee.h"
#include "crc32.h"
#include "trial.h"
#include "layout.h"

#define STATE_LEN 16u

int trial_check(const img_hdr_t *h)
{
    uint8_t b[STATE_LEN];
    uint32_t attempts = 0, i;
    if (ee_read(EE_STATE, b, STATE_LEN)) return R_EE_IO;
    if (rd32(b) == STATE_MAGIC && rd32(b + 4) == h->seq && crc32(b, 12) == rd32(b + 12)) {
        if (b[9] == STATE_CONFIRMED) return R_NONE;
        attempts = b[8];
    }
    if (attempts >= BOOT_TRIALS) return R_TRIAL_EXHAUSTED;
    for (i = 0; i < STATE_LEN; i++) b[i] = 0;
    wr32(b, STATE_MAGIC);
    wr32(b + 4, h->seq);
    b[8] = (uint8_t)(attempts + 1u);
    wr32(b + 12, crc32(b, 12));
    if (ee_write(EE_STATE, b, STATE_LEN)) return R_EE_IO;
    return R_NONE;
}
