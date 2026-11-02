#include <string.h>
#include "profile_bose.h"

#define BOSE_AUX_CDB 540             /* stock AUX gain x 1.8621 (+5.4 dB), also in the graph (0x100104EC) */

void profile_bose_stock(profile_t *p)
{
    params_defaults(p);
    p->flags = PF_LOUDNESS | PF_STOCK_GRAPH;
    strcpy(p->name, "Bose d'origine");
    p->aux_gain_cdb = BOSE_AUX_CDB;
}
