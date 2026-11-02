/*
 * Hand-over to the boot ROM. Sequence from re/tas_rom/NOTES.md 3.2, checked against the ROM disassembly:
 *   DFU_EnterMode 0x8006: stores R7 (mode) at XDATA FA5A and the R3:R2:R1 generic pointer at FA5B..FA5D
 *     with MOVX @R0 (page = P2, so P2 must be 0xFA); returns at once unless FA39 is 0 or 5 (0x809E..0x80AB);
 *     with FA39 = 0 and FBFE = 2, copies VID/PID from the header copy FBF4..FBF7 (big-endian) and MaxPower
 *     FBFB into its device descriptor (0x80AD..0x80E3); FBFA bit 0 = 0: no strings (0x80E8);
 *     copies the 28-byte poll-timeout table from our pointer (0x8235) *before* clearing MEMCFG.SDW (0x8273,
 *     mode 1 only), then reconnects (9DD8: USBCTL = C0). Mode 1 never writes the EEPROM (NOTES.md 4).
 *   BOOT_RebootToRom 0x9F41: IE = 0, USBCTL.SDW_OK, MEMCFG.SDW = 0, LJMP 0x8000 (cold start, re-reads EEPROM).
 * Before either: interrupts off, codec port / DMA / endpoints stopped so that nothing writes into the ROM
 * work areas (FA10..FDAF), D+ pull-up released ~200 ms because the ROM does not disconnect before
 * reconnecting (NOTES.md 3.2 steps 1-2).
 */
#include <8051.h>

#include "config.h"
#include "rom.h"
#include "tas1020b.h"
#include "tick.h"

#define HDR ((volatile __xdata uint8_t *)0xFBF0)    /* ROM copy of the 18-byte EEPROM header */

/*
 * Poll-timeout table for DFU_EnterMode: 7 entries [t0 t1 t2 state]. The ROM loads t0..t2 MSB first as its
 * SOF countdown, but GETSTATUS returns the same bytes to the host, which reads them little-endian
 * (DFU_LoadPollTimeoutForState 0x9CC2 vs 0x8AA5; default table 0x96F8 gives 2097 s to the host). With
 * palindromic values both read the same number: 00 01 00 = 256 ms for MANIFEST, 0 elsewhere (NOTES.md 3.2).
 */
static __code const uint8_t dfu_poll_table[28] = {
    0x00, 0x00, 0x00, 0x02,
    0x00, 0x00, 0x00, 0x03,
    0x00, 0x00, 0x00, 0x05,
    0x00, 0x00, 0x00, 0x06,
    0x00, 0x00, 0x00, 0x09,
    0x00, 0x00, 0x00, 0x0A,
    0x00, 0x01, 0x00, 0x07,
};

static __code const uint8_t dfu_header[18] = {
    0x6A, 0x12, 0x12, 0x34,
    DFU_VID >> 8, DFU_VID & 0xFF, DFU_PID >> 8, DFU_PID & 0xFF,
    0x0A, 0x04,
    0x04,                                       /* USB attributes: self-powered, bit 0 = 0: no strings */
    0x01,                                       /* MaxPower */
    0x08, 0x20,
    0x02,                                       /* DataType 2 */
    0x00, 0x15, 0xE4,
};

/* Stops everything that runs on its own, powers the audio stage down and leaves the bus. */
static void quiesce(void)
{
    uint8_t a;
    EA = 0;
    IE = 0;
    DMACTL0 = 0;
    GLOBCTL = GLOBCTL_MCUCLK | GLOBCTL_LPWR;    /* CPTEN = 0, VREN = 0 (power stage off), RESET = 0 (DSP reset) */
    /* EP_CNF of IN endpoints 7..1 (0xFF30..0xFF60) and OUT endpoints 7..1 (0xFF70..0xFFA0), stride 8
     * (tas1020b.h IEP_BASE / OEP_BASE); 0xFF68 is IN endpoint 0, left to the ROM. */
    for (a = (uint8_t)IEP_BASE(7); a <= (uint8_t)OEP_BASE(1); a += 8) {
        if (a != (uint8_t)IEP_BASE(0)) {
            XREG(0xFF00 | a) = 0;
        }
    }
    USBIMSK = 0;
    USBCTL = 0;                                 /* D+ pull-up off */
    VECINT = 0;
    spin_ms(200);
}

void rom_enter_dfu(void)
{
    uint8_t i;

    quiesce();
    /* Header copy as if the EEPROM held a "device EEPROM" header (DataType 2, NOTES.md 3.2 step 3); the other
     * bytes are those of the stock header (doc.md, TAS1020B firmware section). The checksum is not checked here. */
    for (i = 0; i < sizeof(dfu_header); i++) {
        HDR[i] = dfu_header[i];
    }
    XREG(0xFA39) = 0;                           /* ROM mode: 0 = start-up, so DFU_EnterMode uses its own descriptors */

    __asm
        mov  psw, #0x00
        mov  sp, #0x7F
        mov  0xA0, #0xFA                         ; P2 = 0xFA: ROM accesses its XDATA page with MOVX @Ri
        mov  r7, #0x01                          ; mode 1: download to RAM, never to the EEPROM
        mov  r3, #0xFF                          ; Keil generic pointer to code space
        mov  r2, #>_dfu_poll_table               ; high byte
        mov  r1, #<_dfu_poll_table               ; low byte
        ljmp 0x8006                             ; DFU_EnterMode, does not return
    __endasm;
}

void rom_reboot(void)
{
    quiesce();
    __asm
        ljmp 0x9F41                             ; BOOT_RebootToRom (ROM API index 2)
    __endasm;
}
