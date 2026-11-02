/*
 * Control pod (stock DebounceMuteInput 0x13CD, DetectMuteLine 0x13FD).
 * Mute: P1.3 sampled every ms in Timer 1, 5 identical samples change the debounced state.
 * Presence: the pod powers up muted, so P1.3 going high shortly after start-up means a pod is present.
 * Boot-time DFU escape: the mute pad is a toggle that starts "muted", so "held at power-on" cannot be detected;
 * instead ESCAPE_TOGGLES changes within ESCAPE_WINDOW_MS after detection. rom_enter_dfu() never writes the EEPROM.
 */
#include <8051.h>

#include "pod.h"
#include "rom.h"
#include "tick.h"

#define ESCAPE_TOGGLES     4
#define ESCAPE_WINDOW_MS   8000
#define POD_DETECT_MS      1000    /* stock DetectMuteLine: 40 ms, then up to 50000 polls (~1 s at 24 MHz, [I]) */

__bit pod_present;
__bit pod_mute;
__bit pod_mute_dirty;
__bit pod_plugged;                      /* pod detected after power-up: 'X' 01 to send (0.5.3) */
__bit pod_escape_armed;

/* Updated by POD_DEBOUNCE_TICK() in the Timer 1 interrupt (pod.h). */
__bit pod_isr_last;
__bit pod_isr_state;
volatile __bit pod_isr_changed;
__data uint8_t pod_isr_count;

static __data uint16_t escape_start;
static __data uint8_t escape_count;
static __bit escape_synced;             /* debounced state has reached "muted" since arming */

void pod_detect(void)
{
    static uint8_t n;                           /* static: no push/pop around the calls (SDCC) */
    pod_present = 0;
    delay_ms(40);
    for (n = POD_DETECT_MS / 4; n; n--) {
        if (P1_3) {
            pod_present = 1;
            pod_escape_armed = 1;
            escape_synced = pod_isr_state;      /* the power-up edge 0 -> 1 is not a touch */
            escape_start = tick_now();
            return;
        }
        delay_ms(4);
    }
}

void pod_poll(void)
{
    uint8_t changed;

    ET1 = 0;
    changed = pod_isr_changed;
    pod_isr_changed = 0;
    ET1 = 1;
    if (pod_escape_armed && tick_since(escape_start) > ESCAPE_WINDOW_MS) {
        pod_escape_armed = 0;
    }
    if (!changed) {
        return;
    }
    pod_mute = pod_isr_state;
    pod_mute_dirty = 1;
    /* Re-plug (0.5.3): a pod plugged after power-up powers its touch sensor, which starts "muted" (manual
     * "Sensor"): the line goes high, which no absent pod can do (stock DetectMuteLine relies on the same level).
     * The DSP gets 'X' 01 (dsp_link_poll), then the 'M'. Hot-unplug cannot be seen here: an absent pod and an
     * unmuted one both read low; the DSP's "knob valid" flag covers it (README "No pod"). */
    if (pod_mute && !pod_present) {
        pod_present = 1;
        pod_plugged = 1;
    }
    if (pod_escape_armed) {
        if (!escape_synced) {
            escape_synced = pod_mute;
        } else if (++escape_count >= ESCAPE_TOGGLES) {
            rom_enter_dfu();
        }
    }
}
