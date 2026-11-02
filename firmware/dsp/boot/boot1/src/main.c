/* boot1: second boot stage, stored once at EE_BOOT1 and never rewritten.
 *
 * boot1 never writes the EEPROM except the boot-state page of a trial image. Updates are done by the RAM-only
 * updater (../../updater), loaded through the rescue RAM protocol, so the EEPROM-writing code stays fixable. */
#include "boot.h"
#include "ee.h"
#include "image.h"
#include "trial.h"
#include "rescue.h"

/* Build tag, checked by the host tool before it puts a binary in the boot1 area. */
const char boot_tag[8] = "OCBOOT1";
const uint8_t g_mode = MODE_RESCUE;

uint8_t g_boot_reason;

unsigned cmd_ext(const uint8_t *msg, unsigned len, uint8_t *out)
{
    (void)msg; (void)len; (void)out;
    return 0;                                   /* rescue: RAM loader protocol only */
}

static int try_image(void)
{
    img_hdr_t h;
    int r = image_read_header(&h);
    if (r == R_NONE) r = image_load(&h);
    if (r == R_NONE && (h.flags & HF_TRIAL)) r = trial_check(&h);
    if (r == R_NONE) start_app(h.entry);         /* does not return */
    return r;
}

void boot1_main(void)
{
    g_boot_reason = R_NONE;
    g_in_window = 0;
    g_rescue_requested = 0;
    pll_init();
    ee_init();
    if (request_soft())
        g_boot_reason = R_SOFT_REQUEST;
    else if (boot_window())
        g_boot_reason = R_TAS_REQUEST;
    else
        g_boot_reason = (uint8_t)try_image();
    rescue_run();
}
