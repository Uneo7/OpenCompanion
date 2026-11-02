/* Adopts the TAS link state captured by the RAM loader (include/handoff.h). Called once after audio_init().
 * On a cold boot the record is normally invalid and nothing changes: the TAS then sends X/U/M/V itself. */
#include "oc.h"
#include "handoff.h"
#include "volume.h"

void handoff_apply(void)
{
    volatile handoff_t *h = HANDOFF;
    float raw;
    if (h->magic != HANDOFF_MAGIC || h->inv != ~HANDOFF_MAGIC ||
        h->check != (HANDOFF_MAGIC ^ handoff_word(&h->pod_present) ^ handoff_word(&h->usb_pct))) return;
    /* Not the pod presence (0.2.4): the stock state may be stale (pod unplugged since), and the knob would be
     * reported as valid; the TAS sends 'X' itself. */
    g_audio.usb_up = h->usb_up;
    g_audio.usb_mute = h->usb_mute;
    g_audio.pod_mute = h->pod_mute;
    raw = h->usb_pct * 327.68f - 32768.0f;         /* inverse of the stock 'V' handler */
    if (raw >= -32768.0f && raw <= 32767.0f)
        audio_set_volume_raw((int16_t)(raw + (raw < 0.0f ? -0.5f : 0.5f)));
    h->magic = 0;                                  /* use once: never adopt a stale record after a reset */
}
