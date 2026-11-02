/* Trial-boot counter in the boot-state page (EE_STATE), only when the header has HF_TRIAL.
 * Page format: "OCBS" magic, seq (LE32, = header seq), attempts (u8), confirmed (u8, 0xA5 = confirmed),
 * 2 reserved bytes, CRC-32 of bytes 0..11 (LE32). */
#ifndef BOOT_TRIAL_H
#define BOOT_TRIAL_H

#include "image.h"

#define STATE_MAGIC     0x5342434Fu /* "OCBS" */
#define STATE_CONFIRMED 0xA5u
#define BOOT_TRIALS     2u          /* unconfirmed starts allowed before rescue */

int trial_check(const img_hdr_t *h);   /* R_NONE (boot allowed, attempt recorded) or R_TRIAL_EXHAUSTED */

#endif
