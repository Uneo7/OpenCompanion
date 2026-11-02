/* Boot window: before touching the image, boot1 listens on I2C0 for ~0.1 s so that a TAS firmware can force
 * rescue mode by sending 'E' 7E 'O' 'C' 'R'. DSPINT is NOT pulled low here, so the stock TAS (still waiting in
 * InitDspLink) sends nothing; after a soft reset the stock TAS may poll 'S', which is answered normally.
 * WINDOW_POLLS is a loop count, not a timer (~60 cycles per poll at 245.76 MHz: ~0.1 s, to measure). */
#include "rescue.h"
#include "link.h"

#ifndef WINDOW_POLLS
#define WINDOW_POLLS 400000u
#endif

int g_in_window;
int g_rescue_requested;

int boot_window(void)
{
    uint32_t n;
    g_in_window = 1;
    link_init();
    for (n = 0; n < WINDOW_POLLS && !g_rescue_requested; n++)
        link_poll();
    for (n = 0; n < WINDOW_POLLS && !g_rescue_requested && !link_idle(); n++)
        link_poll();                            /* let a transaction in progress finish */
    g_in_window = 0;
    if (g_rescue_requested)
        return 1;                               /* keep the link: the reply is still to be read */
    link_shutdown();
    return 0;
}
