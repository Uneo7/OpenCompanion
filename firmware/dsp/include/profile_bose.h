/* Built-in read-only profile "Bose d'origine" (OPK): the stock Companion 5 sound (firmware D01.01.01) is the
 * stock graph engine (src/bose/, flag PF_STOCK_GRAPH), with the stock volume law and the stock dynamic loudness
 * (PF_LOUDNESS; the volume is the OS UAC dB value as in every profile) and the stock AUX gain. Its EQ bands are flat: they are only used when the
 * engine cannot run (graph tables missing from the EEPROM, include/bose_tables.h; or PF_BYPASS), where the
 * profile falls back to the neutral preset, flat, with the stock crossover and protection. Raw biquad pairs (the
 * former approximation of the stock sound) are gone (profile protocol 6): params_sanitize() turns them off. */
#ifndef PROFILE_BOSE_H
#define PROFILE_BOSE_H

#include "params.h"

void profile_bose_stock(profile_t *p);

#endif
