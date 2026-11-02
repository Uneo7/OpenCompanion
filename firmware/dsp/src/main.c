#include "oc.h"
#include "reboot_seq.h"
#include "profiles.h"
#include "eeprom.h"
#include "ramp.h"
#include "volume.h"
#include "bootctl.h"
#include "bose_tables.h"

volatile int g_reboot_request;

void isr_global_enable(void);

/* Bring-up stages (make STAGE=n), to find which block breaks on the real hardware:
 *   1 = I2C slave + console only (PLL left as configured by the stock firmware, no audio)
 *   2 = 1 + PLL re-initialisation
 *   3 = 2 + McASP1 configured and started, DMA requests off, dMAX not used
 *   4 = full firmware (McASP1 + dMAX ping-pong audio)
 * Global interrupts are enabled BEFORE the audio start, so the I2C link (and RST) keeps working even if a
 * later step misbehaves. */
#ifndef OC_STAGE
#define OC_STAGE 4
#endif

int main(void)
{
    int cold;
#if OC_STAGE >= 2
    pll_init();
#endif
#if OC_STAGE >= 3
    dmax_hw_init();           /* dMAX program + cores, every start (stock 0x1001D480); cold boot needs it */
#endif
    cold = mcasp0_gpio_init();
    i2c_slave_init();
    isr_init();
    isr_global_enable();
    if (cold)
        mcasp0_dspint_ready();   /* tell the TAS the I2C link is up (stock main 0x10019D08) */
    bootctl_init(cold);
#if OC_STAGE >= 3
    audio_init();
    if (!cold)
        handoff_apply();      /* RAM loader start only: TAS link state of the stock firmware (handoff.h); a
                               * record left in RAM is never adopted on a cold boot (boot1 / ROM) */
    bose_tables_init();       /* stock graph tables: RAM (RAM loader) or EEPROM graph data pages (~25 ms); before
                               * the boot profile, whose EQ depends on them (Bose base) */
    profiles_init();          /* EEPROM scan + boot profile (blocking, ~0.2 s) */
    mcasp1_init();
#endif
#if OC_STAGE == 3
    mcasp1_start_no_dma();
#elif OC_STAGE >= 4
    audio_io_init();          /* buffers + dMAX transfer entries */
    dmax_enable_audio();
    mcasp1_start();
    ramp_init();              /* McASP0 ramp converter: pod knob, bass knob, VRAW (stock 0x10019418) */
#endif

    for (;;) {
        audio_io_service();   /* also reads the GPIO inputs and runs the standby timer (inputs.h) */
#if OC_STAGE >= 4
        ramp_poll();
#endif
        i2c_slave_poll();
#if OC_STAGE >= 3
        profiles_poll();
        bootctl_poll();       /* UPD, trial confirmation (never overlaps a profile job) */
#endif
        if (g_reboot_request) {
            /* Reboot once the TAS has read the reply and no EEPROM transfer is running (an aborted read
             * could leave the EEPROM holding SDA for the ROM boot loader), or after a delay anyway. */
            static unsigned wait;
            if ((i2c_reply_drained() && !eeprom_busy()) || ++wait > 20000000u)
                reboot_to_rom();
        }
    }
}
