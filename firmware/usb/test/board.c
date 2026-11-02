/* Board-level setup for the Bose Companion 5 USB controller (TAS1020B). */
#include <8051.h>

#include "board.h"
#include "tas1020b.h"

/*
 * GLOBCTL: 24 MHz MCU clock, USB blocks powered. The VREN (bit 4) and RESET (bit 3) GPIO outputs stay low:
 * on this board they are understood to drive the power-supply enable (VRAWON) and the DSP reset, so the
 * amplifiers stay unpowered and the DSP stays in reset while this test firmware runs (silent, safe).
 */
void board_init(void)
{
    EA = 0;
    GLOBCTL = GLOBCTL_MCUCLK | GLOBCTL_LPWR;
}

/* Rough busy-wait, calibrated for a 24 MHz 8052 core (not cycle-exact). */
void delay_ms(uint16_t ms)
{
    while (ms--) {
        volatile uint16_t i;
        for (i = 0; i < 1200; i++) {
        }
    }
}

/* Finds "MOVX @DPTR,A ; RET" (F0 22) in the boot ROM, which stays mapped at 0x8000..0x9FFF. */
static uint16_t find_rom_gadget(void)
{
    __code const uint8_t *p = (__code const uint8_t *)0x8000;
    uint16_t i;
    for (i = 0; i < 0x1FFF; i++) {
        if (p[i] == 0xF0 && p[i + 1] == 0x22) {
            return 0x8000 + i;
        }
    }
    return 0;
}

static __data uint8_t gadget_lo, gadget_hi;

/*
 * Returns to the boot ROM. Clearing MEMCFG.SDW maps the ROM back at 0x0000, so the write must not be
 * followed by fetches from our own (now hidden) code: it is executed by a 2-byte gadget in the ROM
 * mirror at 0x8000, whose RET then lands on 0x0000, the ROM reset entry. With the EEPROM header
 * DataType = 0x02 the ROM comes back in DFU mode. If no gadget is found, the firmware disconnects
 * and halts: power-cycle the speaker to get back to DFU.
 */
void reboot_to_rom(void)
{
    uint16_t gadget = find_rom_gadget();

    EA = 0;
    USBCTL &= (uint8_t)~(USBCTL_CONT | USBCTL_FEN);
    delay_ms(50);
    if (!gadget) {
        for (;;) {
        }
    }
    gadget_lo = (uint8_t)gadget;
    gadget_hi = (uint8_t)(gadget >> 8);
    USBCTL |= USBCTL_SDW_OK;

    __asm
        mov  dptr, #0xFFB0      ; MEMCFG
        clr  a                  ; SDW = 0 (other bits are read-only)
        mov  r0, #0x00
        push ar0                ; return address of the gadget: 0x0000 (low byte first)
        push ar0
        push _gadget_lo         ; gadget address, consumed by the RET below
        push _gadget_hi
        ret                     ; -> gadget: MOVX @DPTR,A ; RET -> 0x0000 (ROM)
    __endasm;
}
