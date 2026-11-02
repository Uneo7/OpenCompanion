/* Speaker / headphone mode sequencer and the AXR0[14] amplifier control (src/control/spk_mode.c).
 *
 * Port of three stock pieces (re/dsp/notes/hardware.md section 6bis):
 *  - mode selector, class 0x1001BF14, graph instance 0x10010164, run every block inside the graph:
 *    input 0 = pod mute 0x10010114 (1.0 = muted / headphones, 2.0 = normal; stock init 1.0), input 2 = the
 *    acknowledge 0x1001011C written back by 0x100157E8; output 1 (the state) -> MD 0x10010120;
 *  - 0x100157E8 (frame task 0x10015AE8, after the graph): AXR0[14] from MD, and the standby release;
 *  - 0x10015910 (same task, next call): AXR0[14] release on a supply / clip fault.
 * The 'm' USB mute does not reach this logic in stock (it only zeroes the USB volume curve, 0x10010108). */
#ifndef SPK_MODE_H
#define SPK_MODE_H

#include <stdint.h>

/* Selector states = MD (stock 'SYS' prints it as MD). */
enum {
    SPK_SWITCHING = 0,      /* routing being changed (stock T2 + 1 = 8 blocks) */
    SPK_TO_MUTED  = 1,      /* AXR0[14] released, waits for the acknowledge, then 31 blocks */
    SPK_TO_ON     = 2,      /* AXR0[14] driven low, waits for the acknowledge, then 31 blocks */
    SPK_MUTED     = 3,      /* headphones / pod mute, speakers off */
    SPK_ON        = 4       /* speakers on */
};

typedef struct {
    uint8_t  state;         /* stock instance word 10 (0x1001018C) -> MD 0x10010120 */
    uint8_t  ack;           /* stock 0x1001011C: 1 once AXR0[14] was released, 0 once driven */
    uint8_t  rel_done;      /* stock byte 0x540C: MD = 1 handled */
    uint8_t  drv_done;      /* stock byte 0x540D: MD = 2 handled */
    uint8_t  standby_done;  /* release done for the standby request (stock 0x540E set by 0x100157E8) */
    uint8_t  fault_done;    /* release done for the fault (stock 0x540F) */
    uint8_t  driven;        /* last action: 1 = AXR0[14] driven low (diagnostics) */
    uint8_t  pad;
    uint32_t count;         /* stock instance word 14 (0x1001019C) */
} spk_mode_t;
extern spk_mode_t g_spk;

void spk_mode_init(void);
void spk_mode_frame(void);  /* once per audio frame, after the output stage (stock order) */

#endif
