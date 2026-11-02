#ifndef OC_H
#define OC_H

#include <stdint.h>

#ifndef OC_VERSION                 /* set by the Makefile: "OC" FW_VERSION "+" FW_BUILD */
#define OC_VERSION       "OC0.0.1"
#endif
#define FRAME_LEN        96      /* samples per frame (2 ms at 48 kHz), as Bose */
#define TDM_SLOTS        8
#define RX_SERIALIZERS   2       /* serializer 0 = USB (TAS), 1 = codec ADC (AUX) */

/* Input slots (service manual) */
enum { IN_L, IN_R, IN_C, IN_LFE, IN_LS, IN_RS };          /* USB serializer */
enum { IN_AUX_L, IN_AUX_R };                              /* codec serializer */
/* Output slots to the DAC */
enum { OUT_INNER_L, OUT_INNER_R, OUT_OUTER_L, OUT_OUTER_R, OUT_BASS_A, OUT_BASS_B, OUT_COUNT };

/* Ping-pong buffers: [serializer][slot][sample] on receive, [slot][sample] on transmit */
typedef int32_t rx_buf_t[RX_SERIALIZERS][TDM_SLOTS][FRAME_LEN];
typedef int32_t tx_buf_t[TDM_SLOTS][FRAME_LEN];

/* --- mcasp0_gpio.c --- */
int  mcasp0_gpio_init(void);            /* 1 = cold boot */
void mcasp0_dspint_ready(void);
void mcasp0_dspint_release(void);
void mcasp0_amp_drive_low(void);         /* AXR0[14] output low: speakers on (spk_mode.c) */
void mcasp0_amp_release(void);          /* AXR0[14] input: speakers off */

/* --- pll.c --- */
void pll_init(void);

/* --- dmax.c --- */
void dmax_hw_init(void);                /* dMAX program load + core start (stock 0x1001D480), before any PaRAM use */
void dmax_audio_init(int32_t *rx_ping, int32_t *rx_pong, int32_t *tx_ping, int32_t *tx_pong);
void dmax_enable_audio(void);
uint32_t dmax_take_tcc(void);
#define TCC_RX_BIT (1u << 5)
#define TCC_TX_BIT (1u << 2)

/* --- mcasp1.c --- */
void mcasp1_init(void);
void mcasp1_start(void);
void mcasp1_start_no_dma(void);

/* --- audio_io.c --- */
typedef struct {
    uint32_t period;      /* RTI ticks between the last two frames (nominal frame period) */
    uint32_t busy;        /* ticks of the last audio_process() */
    uint32_t max;         /* longest audio_process() since the last CPU read */
    uint32_t avg;         /* smoothed busy ticks (1/16 per frame) */
    uint32_t lost;        /* frames received while the previous one was not processed yet */
} cpu_load_t;
extern cpu_load_t g_cpu;
void audio_io_init(void);
void audio_io_service(void);            /* main loop: process one frame if ready */
void audio_io_on_rx_done(void);         /* called from the dMAX ISR */

/* --- audio.c --- */
typedef struct {
    float volume_db;      /* USB volume after the active volume law */
    float volume_pct;     /* stock percentage of the last 'V' value */
    int16_t volume_raw;   /* last 'V' value (UAC, 1/256 dB) */
    int   volume_known;   /* 0 until the first 'V' (or the RAM loader handoff): VOL_SAFE_DB applies */
    int   usb_up;         /* 'U' received */
    int   stream_open;    /* last 'R' layout byte bit0 (TAS >= 0.4.0): the host streams to the speaker */
    float cap_db;         /* safety ceiling */
    int   usb_mute;
    int   pod_mute;       /* pod mute / headphones plugged */
    int   headphones;     /* headphone feed on (audio.c): speakers released, inner L/R = flat headphone feed */
    int   pod_present;
    uint32_t frames;      /* processed frames */
    uint32_t clips;       /* limited samples */
} audio_state_t;
extern audio_state_t g_audio;

/* Set by the RST console command; the reboot happens once the TAS has read the reply. */
extern volatile int g_reboot_request;
int i2c_reply_drained(void);
void audio_init(void);
void audio_process(const rx_buf_t *in, tx_buf_t *out);
void audio_set_volume_raw(int16_t v);   /* 'V' value (UAC, 1/256 dB) */
#define VOL_SAFE_DB  (-30.0f)    /* master volume before the first 'V', whatever the volume law */
void audio_update_volume(void);         /* re-applies the USB volume law (profile change), volume.c */
void audio_rate(void);                  /* 48 kHz constants */
int  audio_graph_active(void);          /* 1 = the stock graph engine runs (Bose d'origine) */
/* 2.1 routing (engine.c): in the TAS 2.1 mode (rate.h LAYOUT_21) on the neutral preset each outer driver plays
 * its inner driver's signal (inner EQ and crossover; own trim) and the bass gets (L + R) / 2. The Bose preset
 * keeps the stock routing. The TAS 'R' frame is the only source of the 2.1 mode. */
int  audio_route21_active(void);        /* 2.1 routing in effect now */

/* --- meters.c --- */
#define METER_IN_COUNT   8       /* USB L,R,C,LFE,Ls,Rs, AUX L,R */
#define METER_OUT0       METER_IN_COUNT
#define METER_COUNT      (METER_IN_COUNT + OUT_COUNT)
void  meters_init(void);
void  meters_update(int idx, const float *x, int n, float s);   /* block x s */
void  meters_set(int idx, float pk);                             /* block peak */
float meters_peak(int idx, int reset);   /* peak since last reset (linear) */
int   meters_to_db(float lin);           /* integer dBFS, -99 = silence */

/* --- i2c_slave.c --- */
void i2c_slave_init(void);
void i2c_slave_isr(void);               /* called from the INT14 ISR */
void i2c_slave_poll(void);              /* processes received frames (outside interrupts) */

/* --- console.c --- */
int console_exec(const uint8_t *msg, int len, uint8_t *reply, int max);

/* --- isr.c --- */
void isr_init(void);

#endif
