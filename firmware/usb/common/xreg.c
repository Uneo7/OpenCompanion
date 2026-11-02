/*
 * Assembly for size. MOVX @Ri takes its high address byte from P2 on this core. P2 is free in both firmwares
 * (model small, no __pdata, no port 2 pins); the ROM sets its own P2 = 0xFA where it needs it.
 */
#include "xreg.h"

void xreg_load(__code const uint8_t *table) __naked
{
    (void)table;                                /* in DPTR */
    __asm
        mov  0xA0, #0xFF                        ; P2 = register page
    00001$:
        clr  a
        movc a, @a+dptr                         ; register low byte, 0 = end
        jz   00002$
        mov  r0, a
        inc  dptr
        clr  a
        movc a, @a+dptr                         ; value
        inc  dptr
        movx @r0, a
        sjmp 00001$
    00002$:
        ret
    __endasm;
}
