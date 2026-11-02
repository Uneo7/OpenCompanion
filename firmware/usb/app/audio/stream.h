#ifndef STREAM_H
#define STREAM_H

#include <stdint.h>


/* Each mode has its own configuration descriptor with a single streaming alternate setting. */
#define AUDIO_MODE_51  0
#define AUDIO_MODE_21  1
#define AUDIO_MODES    2
/* HID MODE command and EEPROM mode record: code = audio_mode + 1 */
#define MODE_CODE_51   1
#define MODE_CODE_21   2

/* 4th byte of 'R' (board/dsp_link.c) */
#define LAYOUT_STREAM  0x01
#define LAYOUT_MODE21  0x02
#define ALT_COUNT      1                   /* streaming alternate settings per configuration */

/* AudioControl / AudioStreaming interface numbers of the current mode (config.h IF_*_51 / IF_*_21) */
#define IF_AC          (audio_mode)
#define IF_AS          ((uint8_t)(audio_mode + 1))

extern __data uint8_t audio_mode;          /* AUDIO_MODE_* */
extern __data uint8_t stream_alt;          /* interface 1 alternate setting: 0 idle, 1 streaming */
extern __data uint8_t stream_rate;         /* always 0 = 48 kHz since 0.5.1 (INFO byte 8 compatibility) */
extern __data uint8_t stream_slot_mask;    /* bytes per time slot - 1 while streaming (1 or 3), 0 while idle */

void stream_init(void);
void stream_set_alt(uint8_t alt);
void stream_dma(uint8_t on);

#endif /* STREAM_H */
