/*
 * 1 ms tick on Timer 1 mode 1, reloaded in the interrupt like the stock Timer1Isr (0x0BC9): 24 MHz / 12 = 2000
 * counts per ms. The interrupt also debounces the pod mute line.
 */
#include <8051.h>

#include "pod.h"
#include "tick.h"

#define TICK_RELOAD (65536u - 2000u)

static volatile __data uint16_t ticks;

void timer1_isr(void) __interrupt(3)
{
    TR1 = 0;
    TH1 = TICK_RELOAD >> 8;
    TL1 = TICK_RELOAD & 0xFF;
    TR1 = 1;
    ticks++;
    POD_DEBOUNCE_TICK();
}

void tick_init(void)
{
    TR1 = 0;
    TMOD = (TMOD & 0x0F) | 0x10;                /* Timer 1 mode 1, Timer 0 untouched */
    TH1 = TICK_RELOAD >> 8;
    TL1 = TICK_RELOAD & 0xFF;
    TF1 = 0;
    TR1 = 1;
    ET1 = 1;
    EA = 1;                                     /* only Timer 1 is enabled: EX0 (USB) stays off, USB is polled */
}

uint16_t tick_now(void)
{
    uint16_t t;
    ET1 = 0;
    t = ticks;
    ET1 = 1;
    return t;
}

void delay_ms(uint16_t ms)
{
    static uint16_t end;                        /* static: no push/pop around main_idle() (SDCC) */
    end = tick_now() + ms;
    while ((int16_t)(tick_now() - end) < 0) {
        main_idle();
    }
}

void spin_ms(uint16_t ms)
{
    EA = 0;
    while (ms--) {
        TF1 = 0;
        TR1 = 0;
        TH1 = TICK_RELOAD >> 8;
        TL1 = TICK_RELOAD & 0xFF;
        TR1 = 1;
        while (!TF1) {
        }
    }
}
