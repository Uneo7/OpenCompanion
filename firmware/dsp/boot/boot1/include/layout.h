/* EEPROM (U7001, AT24C512: 64 KB, 128-byte pages) and RAM layout of the OpenCompanion boot chain.
 * Specification: ../docs/LAYOUT.md. The host tool (app/crates/oc-ais, module `eeprom`) uses the same values. */
#ifndef BOOT_LAYOUT_H
#define BOOT_LAYOUT_H

/* ---- EEPROM ---- */
#define EE_SIZE         0x10000u
#define EE_PAGE         128u

#define EE_BOOT0        0x0000u     /* AIS: magic + Section Load (trampoline) + Jump_Close. Written once. */
#define EE_BOOT0_END    0x0100u
#define EE_HDR          0x0100u     /* image header page ("OCI1"), written LAST during an update */
#define EE_STATE        0x0180u     /* boot-state page ("OCBS"): trial-boot counter / confirmation */
#define EE_BODY         0x0200u     /* image body: AIS magic, LZ4 sections, Jump_Close */
#define EE_BODY_END     0xC200u
#define EE_BODY_MAX     (EE_BODY_END - EE_BODY)
#define EE_PROFILES     0xC200u     /* profiles (8 x 0x300) + config A/B (2 x 0x80), moved from 0xE000 */
#define EE_PROFILES_END 0xDB00u
#define EE_BOOT1        0xDB00u     /* boot1 (code + const) + CRC-32 in its last 4 bytes. Written once, while */
#define EE_BOOT1_LEN    0x2480u     /* the stock image (which ends at 0xDAD8) still boots: must stay above it */
#define EE_LAST_PAGE    0xFF80u     /* kept as is: byte 0xFFFF = stock "CKT" value */

/* EEPROM writes: [EE_WR_LO, EE_WR_HI) only. The updater may also write boot0 / boot1 after an explicit
 * unlock (migration only). Nothing ever writes the last page. */
#define EE_WR_LO        EE_HDR
#define EE_WR_HI        EE_BOOT1

/* ---- RAM (192 KB, 0x10000000..0x1002FFFF) ---- */
#define RAM_LO          0x10000000u
#define RAM_HI          0x10030000u
#define RAM_TRAMP       0x1002AF00u /* boot0 trampoline (loaded by the ROM, dead once boot1 runs) */
#define RAM_BOOT1       0x1002B000u /* boot1 image as read from EE_BOOT1 (EE_BOOT1_LEN bytes) */
#define RAM_FLAG        0x1002DFF0u /* 2 words: software rescue request (request.c), gap after boot1 */
#define RAM_BOOT1_BSS   0x1002E000u /* boot1 variables and stack, not loaded */
#define RAM_BOOT_LO     RAM_TRAMP   /* [RAM_BOOT_LO, RAM_HI) is never written by an image or by the host */

/* Application images (and the RAM-only updater) may only load into [RAM_LO, RAM_BOOT_LO). */
#define APP_RAM_LO      RAM_LO
#define APP_RAM_HI      RAM_BOOT_LO

#endif
