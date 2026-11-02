#include "oc.h"
#include "rate.h"
#include "audio_out.h"

enum { R_IDLE, R_FADE, R_HOLD };

uint8_t g_layout = LAYOUT_51;
static uint8_t pending_layout = LAYOUT_51;
static int state;
static uint32_t hold;

void rate_init(void)
{
    g_layout = pending_layout = LAYOUT_51;
    state = R_IDLE;
}

/* TAS 0.4.0 'R' layout byte: bit0 stream open, bit1 2.1 mode (24-bit stereo stream, the DSP builds the sub).
 * The mode alone selects the layout (TAS 0.3.0 values 1 / 2 / 3 map to 5.1 / 2.1 / 2.1). */
void layout_request(uint8_t layout)
{
    pending_layout = (layout & 2u) ? LAYOUT_21 : LAYOUT_51;
}

float rate_frame(void)
{
    switch (state) {
    case R_IDLE:
        if (pending_layout == g_layout) return 1.0f;
        state = R_FADE;
        return 0.0f;                   /* the output ramp closes */
    case R_FADE:
        if (!audio_out_silent()) return 0.0f;
        g_layout = pending_layout;
        hold = RATE_HOLD_FRAMES;
        state = R_HOLD;
        return 0.0f;
    default:
        if (pending_layout != g_layout) {   /* changed again while holding */
            state = R_FADE;
            return 0.0f;
        }
        if (hold) { hold--; return 0.0f; }
        state = R_IDLE;
        return 1.0f;
    }
}
