/* Master volume, USB host / local mode, pod knob and pod mute reporting (src/volume.c).
 *
 * Design (OpenCompanion, replaces the stock "pod volume x USB volume" law):
 *  - the OS volume and mute are the single master volume. The pod knob and the pod mute are inputs that the
 *    TAS reports to the host (status bytes below), which moves the OS slider / mute and sends the usual UAC
 *    SET_CUR back ('V', 'm'). In HOST mode the DSP never multiplies the pod percentage into the gain;
 *  - LOCAL mode (no USB host, e.g. AUX only): the pod knob sets the volume through the stock pod curve
 *    (0x1001B140 with the 0x10010564 parameters). Without a pod the volume is the last USB volume;
 *  - HOST vs LOCAL comes from the TAS, as in the stock firmware: after 'U', the TAS drives DSPINT (P1.6 ->
 *    DSP AXR0[9]) low while USB is active (TAS PollUsbVbusAndDspPower 0x0FD7); the stock DSP reads the pin in
 *    every frame (0x10015700..0x100157D4) and mutes the USB channels (0x10010108 = 1.0) while it is high.
 *    Before 'U' the mode is HOST (the DSP drives DSPINT itself, nothing can play anyway);
 *  - the master volume is applied after the USB + AUX mix (it controls AUX too), with the safety ceiling;
 *    the USB mute 'm' mutes the USB channels only (stock); the pod mute 'M' (pod mute touch or headphones)
 *    mutes the speakers in both modes (stock mode selector 0x1001BF14).
 *
 * 'S' status reply (i2c_slave.c), 5 bytes in one frame:
 *   [0] standby request (stock 0x10006D5E)  [1] fault (stock 0x10006D5F)  -- unchanged, read by the TAS
 *   [2] flags: bit0 pod present, bit1 pod muted ('M'), bit2 LOCAL mode, bit3 knob valid, bit7 = 1 (marks
 *       the extended OpenCompanion status; the stock DSP replies with 2 bytes)
 *   [3] pod knob position, 0..99 %: the stock-filtered knob (ramp.c -> volume_curve.c pod_filter: one-pole
 *       0.05 / 0.95 per 2 ms frame, x 99 %, snap / hold 0.25 / 0.75 %, 1 % steps), then a direction-aware
 *       hysteresis (volume_curve.c knob_step: 1 % steps while turning the same way, 4 % to turn back): never
 *       the raw IVOL reading. The same value is the volume in LOCAL mode
 *   [4] event counter, + 1 (mod 256) each time [3] or the pod mute changes */
#ifndef VOLUME_H
#define VOLUME_H

#include <stdint.h>

#define VS_POD_PRESENT  0x01
#define VS_POD_MUTE     0x02
#define VS_LOCAL        0x04
#define VS_KNOB_VALID   0x08
#define VS_EXTENDED     0x80

typedef struct {
    uint8_t host;        /* 1 = HOST mode (volume = USB volume), 0 = LOCAL */
    uint8_t usb_off;     /* USB inactive (DSPINT high after 'U'): USB channels muted, stock 0x10015744 */
    uint8_t knob;        /* reported knob, 0..99 % */
    uint8_t knob_valid;
    uint8_t events;
    float   master_db;   /* applied after the mix, before the ceiling */
} volume_t;
extern volume_t g_vol;

void volume_init(void);
void volume_frame(void);              /* once per audio frame, after gpio_in_frame() */
int  volume_status(uint8_t *st);      /* bytes [2..4] of the 'S' reply; returns 3 */
void handoff_apply(void);             /* src/handoff.c: adopt the RAM loader's copy of the TAS link state */

#endif
