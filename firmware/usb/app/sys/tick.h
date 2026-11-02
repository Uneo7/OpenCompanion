#ifndef TICK_H
#define TICK_H

#include <stdint.h>

void tick_init(void);
/* Timer 1 interrupt (vector 3, code 0x001B). Its prototype must be visible in main.c (SDCC). */
void timer1_isr(void) __interrupt(3);
uint16_t tick_now(void);                 /* milliseconds, wraps every 65.5 s */
#define tick_since(t) ((uint16_t)(tick_now() - (t)))

/* Waits while servicing USB (calls main_idle()); must never be called from a USB callback. */
void delay_ms(uint16_t ms);
/* Busy wait with interrupts off (Timer 1 overflow polled): only for the ROM hand-over paths. */
void spin_ms(uint16_t ms);

/* Provided by main.c: background work allowed during delay_ms (USB polling). */
void main_idle(void);

#endif /* TICK_H */
