#include <string.h>
#include "profiles.h"
#include "eeprom.h"
#include "oc.h"
#include "crc32.h"
#include "sub_level.h"
#include "bose_tables.h"
#include "profile_bose.h"
#include "inputs.h"

#define PROF_BASE     PROF_AREA_LO
#define COPY_SIZE     PROF_COPY_SIZE
#define HDR_SIZE      16u
#define REC_SIZE      (HDR_SIZE + PROFILE_SIZE)        /* 632 bytes = 5 pages */
#define CFG_BASE      PROF_CFG_BASE
#define CFG_STRIDE    PROF_CFG_STRIDE
#define CFG_SIZE      16u
#define MAGIC_PROF    0x3150434Fu                      /* "OCP1" little-endian */
#define MAGIC_CFG     0x3143434Fu                      /* "OCC1" */

/* Compile-time layout checks: slots below the config copies, everything inside the driver write window. */
typedef char layout_slots_check[(PROF_BASE + 2u * PROFILE_SLOTS * COPY_SIZE <= CFG_BASE) ? 1 : -1];
typedef char layout_cfg_check[(CFG_BASE + 2u * CFG_STRIDE <= PROF_AREA_HI) ? 1 : -1];
typedef char layout_window_check[(EE_WRITE_LO == PROF_AREA_LO && EE_WRITE_HI == PROF_AREA_HI) ? 1 : -1];
typedef char layout_rec_check[(HDR_SIZE + PROFILE_SIZE <= COPY_SIZE && CFG_SIZE <= CFG_STRIDE) ? 1 : -1];
/* Layout v3: a record never reaches the graph data page of its copy (include/bose_tables.h). */
typedef char layout_v3_check[(REC_SIZE <= BT_PAGE_OFF && BT_PAGE_OFF + EE_PAGE == COPY_SIZE) ? 1 : -1];

/* EEPROM layout markers (boot/docs/RESCUE.md, "Changes needed in the DSP firmware" #3; LAYOUT.md v3): v2 =
 * profiles at 0xC200, never over boot1; v3 = v2 + the graph data pages are reserved (never written here) and
 * hold .bose_tab. */
#pragma RETAIN(oc_eeprom_layout_v2)
const uint32_t oc_eeprom_layout_v2 = 2u;
#pragma RETAIN(oc_eeprom_layout_v3)
const uint32_t oc_eeprom_layout_v3 = 3u;

#define DOUBLE_PRESS_FRAMES 300u                       /* 600 ms between the two pod mute presses */

enum { J_NONE, J_SAVE, J_LOAD, J_CFG };

typedef struct {
    uint32_t seq;
    uint8_t  valid;        /* magic + length + version + CRC OK */
    uint8_t  writable;     /* blank or OpenCompanion magic */
} copy_state_t;

static copy_state_t copies[PROFILE_SLOTS][2], cfg_copies[2];
static slot_info_t slots[PROFILE_SLOTS];
static int active = SLOT_NONE, boot_slot = SLOT_NONE, modified, last_result = PR_OK;
static uint32_t edits, edits_at_save;

static uint8_t buf[REC_SIZE];          /* record being saved / loaded */
static uint8_t dev[3];                 /* device settings = config record bytes 9..11: sub level source, sub level
                                        * (SUB_LEVEL_CODE), options (OPT_*); 0 = default, also in older records */
static int cfg_dirty;                  /* a debounced config write is pending (sub level) */
static int write_cfg(int slot);
static uint32_t cfg_due;               /* audio frame of that write */
static uint8_t chunk[EE_PAGE];         /* read-back chunk */
static profile_t tmp_profile;

static struct {
    int kind, slot, copy;
    uint16_t base, size;
    int step, nsteps;                  /* writes (pages), then read-back chunks */
    int npages;
    uint32_t seq;
} job;

static uint16_t copy_addr(int slot, int copy) { return (uint16_t)(PROF_BASE + (2u * slot + copy) * COPY_SIZE); }
static uint16_t cfg_addr(int copy) { return (uint16_t)(CFG_BASE + copy * CFG_STRIDE); }

static int all_ff(const uint8_t *p, uint32_t n)
{
    while (n--) if (*p++ != 0xFF) return 0;
    return 1;
}

static void classify_record(copy_state_t *cs)
{
    uint32_t magic = oc_get32(buf);
    cs->writable = (magic == MAGIC_PROF) || all_ff(buf, REC_SIZE);
    cs->valid = magic == MAGIC_PROF && (buf[8] | (buf[9] << 8)) == PROFILE_SIZE && buf[10] == PROFILE_VERSION &&
                oc_get32(buf + 12) == oc_crc32(buf + HDR_SIZE, PROFILE_SIZE);
    cs->seq = cs->valid ? oc_get32(buf + 4) : 0;
}

static void classify_cfg(copy_state_t *cs, const uint8_t *p)
{
    uint32_t magic = oc_get32(p);
    cs->writable = (magic == MAGIC_CFG) || all_ff(p, CFG_SIZE);
    cs->valid = magic == MAGIC_CFG && oc_get32(p + 12) == oc_crc32(p, 12);
    cs->seq = cs->valid ? oc_get32(p + 4) : 0;
}

static int newest(const copy_state_t *c)
{
    if (c[0].valid && (!c[1].valid || (int32_t)(c[0].seq - c[1].seq) >= 0)) return 0;
    return c[1].valid ? 1 : -1;
}

static void refresh_slot_info(int s, const profile_t *p)
{
    int c = newest(copies[s]);
    slots[s].valid = c >= 0;
    slots[s].writable = copies[s][0].writable && copies[s][1].writable;
    slots[s].seq = c >= 0 ? copies[s][c].seq : 0;
    if (p) {
        memcpy(slots[s].name, p->name, PROFILE_NAME_LEN);
        slots[s].color = p->color;
    } else if (c < 0) {
        memset(slots[s].name, 0, PROFILE_NAME_LEN);
        slots[s].color = 0;
    }
}

/* Built-in "Bose d'origine" (Bose base, flat EQ) as the active profile of slot s (SLOT_NONE = unsaved). */
static int builtin(int s)
{
    profile_bose_stock(&tmp_profile);
    params_set_active(&tmp_profile);
    active = s;
    modified = 0;
    return PR_OK;
}

static void apply_dev(void)
{
    sub_level_set_source(dev[0]);
    sub_level_set_app_db(SUB_LEVEL_SET(dev[1]) ? (float)((int)dev[1] - 128) * 0.1f : 0.0f);   /* unset: 0 dB */
    standby_config(dev[2]);
}

#define read_blocking eeprom_read_wait

void profiles_init(void)
{
    int s, c;
    uint8_t cfg[CFG_SIZE];

    eeprom_init();
    for (s = 0; s < PROFILE_SLOTS; s++) {
        int best;
        for (c = 0; c < 2; c++) {
            if (read_blocking(copy_addr(s, c), buf, REC_SIZE)) {
                classify_record(&copies[s][c]);
            } else {
                copies[s][c].valid = 0;
                copies[s][c].writable = 0;     /* unreadable: never write */
                copies[s][c].seq = 0;
            }
        }
        best = newest(copies[s]);
        refresh_slot_info(s, 0);
        if (best >= 0 && read_blocking(copy_addr(s, best), buf, REC_SIZE)) {
            memcpy(&tmp_profile, buf + HDR_SIZE, PROFILE_SIZE);
            refresh_slot_info(s, &tmp_profile);
        }
    }
    for (c = 0; c < 2; c++) {
        if (read_blocking(cfg_addr(c), cfg, CFG_SIZE)) classify_cfg(&cfg_copies[c], cfg);
        else cfg_copies[c].valid = cfg_copies[c].writable = 0;
    }
    c = newest(cfg_copies);
    if (c >= 0 && read_blocking(cfg_addr(c), cfg, CFG_SIZE)) {
        boot_slot = cfg[8] < PROFILE_SLOTS ? cfg[8] : SLOT_NONE;
        memcpy(dev, cfg + 9, sizeof(dev));             /* 0 (older records, default) = knob, off, stock */
        apply_dev();
    }

    if (boot_slot != SLOT_NONE && slots[boot_slot].valid &&
        read_blocking(copy_addr(boot_slot, newest(copies[boot_slot])), buf, REC_SIZE)) {
        copy_state_t cs;
        classify_record(&cs);
        if (cs.valid) {
            memcpy(&tmp_profile, buf + HDR_SIZE, PROFILE_SIZE);
            params_set_active(&tmp_profile);
            active = boot_slot;
        }
    }
    if (active == SLOT_NONE) builtin(SLOT_NONE);       /* no boot slot (FF): built-in Bose base, flat EQ */
    modified = 0;
    job.kind = J_NONE;
}

static int start_job(int kind, int slot, int copy, uint16_t base, uint16_t size, int writes)
{
    job.kind = kind;
    job.slot = slot;
    job.copy = copy;
    job.base = base;
    job.size = size;
    job.npages = (size + EE_PAGE - 1) / EE_PAGE;
    job.nsteps = writes ? 2 * job.npages : 1;       /* writes + read-back chunks, or one read */
    job.step = 0;
    last_result = PR_BUSY;
    return PR_OK;
}

static void end_job(int result)
{
    int s = job.slot;
    if (job.kind == J_SAVE) {
        copies[s][job.copy].valid = result == PR_OK;
        copies[s][job.copy].seq = result == PR_OK ? job.seq : 0;
        refresh_slot_info(s, 0);
        if (result == PR_OK) {
            memcpy(&tmp_profile, buf + HDR_SIZE, PROFILE_SIZE);
            refresh_slot_info(s, &tmp_profile);
            active = s;
            modified = edits != edits_at_save;
        }
    } else if (job.kind == J_LOAD) {
        if (result == PR_OK) {
            copy_state_t cs;
            classify_record(&cs);
            if (cs.valid) {
                memcpy(&tmp_profile, buf + HDR_SIZE, PROFILE_SIZE);
                params_set_active(&tmp_profile);
                active = s;
                modified = 0;
            } else {
                result = PR_ERR_CRC;
            }
        }
    } else if (job.kind == J_CFG) {
        cfg_copies[job.copy].valid = result == PR_OK;
        cfg_copies[job.copy].seq = result == PR_OK ? job.seq : 0;
        if (result == PR_OK) boot_slot = buf[8] < PROFILE_SLOTS ? buf[8] : SLOT_NONE;
    }
    last_result = result;
    job.kind = J_NONE;
}

/* Page written at write step k: profile pages first, the header page (0) last. */
static int page_of_step(int k) { return k + 1 < job.npages ? k + 1 : 0; }

static int issue_op(void)
{
    int k = job.step;
    if (job.kind == J_LOAD)
        return eeprom_read(job.base, buf, job.size);
    if (k < job.npages) {                              /* write phase */
        uint16_t off = (uint16_t)(page_of_step(k) * EE_PAGE);
        uint16_t n = (uint16_t)(job.size - off < EE_PAGE ? job.size - off : EE_PAGE);
        return eeprom_write((uint16_t)(job.base + off), buf + off, n);
    } else {                                           /* read-back phase */
        uint16_t off = (uint16_t)((k - job.npages) * EE_PAGE);
        uint16_t n = (uint16_t)(job.size - off < EE_PAGE ? job.size - off : EE_PAGE);
        return eeprom_read((uint16_t)(job.base + off), chunk, n);
    }
}

static int check_step(void)
{
    int k = job.step;
    if (job.kind != J_LOAD && k >= job.npages) {
        uint16_t off = (uint16_t)((k - job.npages) * EE_PAGE);
        uint16_t n = (uint16_t)(job.size - off < EE_PAGE ? job.size - off : EE_PAGE);
        if (memcmp(chunk, buf + off, n)) return PR_ERR_VERIFY;
    }
    return PR_OK;
}

static uint32_t pod_last_edge;
static int pod_prev, pod_presses;

static void pod_double_press(void)
{
    /* Two pod mute presses (mute on, then off) within DOUBLE_PRESS_FRAMES cycle the stored profiles. */
    int m = g_audio.pod_mute;
    if (m == pod_prev) {
        if (pod_presses && g_audio.frames - pod_last_edge > DOUBLE_PRESS_FRAMES) pod_presses = 0;
        return;
    }
    pod_prev = m;
    if (pod_presses && g_audio.frames - pod_last_edge <= DOUBLE_PRESS_FRAMES) {
        pod_presses = 0;
        if (!m && !(dev[2] & OPT_NO_DTAP)) profiles_cycle();   /* second press, back to unmuted */
    } else {
        pod_presses = m ? 1 : 0;
    }
    pod_last_edge = g_audio.frames;
}

void profiles_poll(void)
{
    int r;
    pod_double_press();
    if (cfg_dirty && job.kind == J_NONE && !eeprom_busy() && (int32_t)(g_audio.frames - cfg_due) >= 0)
        cfg_dirty = write_cfg(boot_slot) == PR_BUSY;   /* done, or not ours (PR_ERR_PROTECTED): stop trying */
    if (job.kind == J_NONE) return;
    r = eeprom_poll();
    if (r == EE_BUSY) return;
    if (r == EE_ERROR) { end_job(PR_ERR_IO); return; }
    if (r == EE_DONE) {
        int c = check_step();
        if (c != PR_OK) { end_job(c); return; }
        if (++job.step >= job.nsteps) { end_job(PR_OK); return; }
    }
    if (!eeprom_busy() && issue_op()) end_job(PR_ERR_IO);
}

int profiles_save(int slot)
{
    int c;
    if (slot < 0 || slot >= PROFILE_SLOTS) return PR_ERR_ARG;
    if (job.kind != J_NONE || eeprom_busy()) return PR_BUSY;
    if (!slots[slot].writable) return last_result = PR_ERR_PROTECTED;
    c = newest(copies[slot]);
    c = c < 0 ? 0 : c ^ 1;                             /* never overwrite the newest valid copy */
    job.seq = slots[slot].valid ? slots[slot].seq + 1 : 1;
    oc_put32(buf, MAGIC_PROF);
    oc_put32(buf + 4, job.seq);
    buf[8] = (uint8_t)PROFILE_SIZE;
    buf[9] = (uint8_t)(PROFILE_SIZE >> 8);
    buf[10] = PROFILE_VERSION;
    buf[11] = 0;
    memcpy(buf + HDR_SIZE, &g_profile, PROFILE_SIZE);
    oc_put32(buf + 12, oc_crc32(buf + HDR_SIZE, PROFILE_SIZE));
    edits_at_save = edits;
    copies[slot][c].valid = 0;                         /* being rewritten */
    return start_job(J_SAVE, slot, c, copy_addr(slot, c), REC_SIZE, 1);
}

int profiles_load(int slot)
{
    int c;
    if (slot < 0 || slot >= PROFILE_SLOTS) return PR_ERR_ARG;
    if (job.kind != J_NONE || eeprom_busy()) return PR_BUSY;
    c = newest(copies[slot]);
    if (c < 0 && slot == 0) return last_result = builtin(0);   /* slot 1 never saved: starts as the Bose base */
    if (c < 0) return last_result = PR_ERR_EMPTY;
    return start_job(J_LOAD, slot, c, copy_addr(slot, c), REC_SIZE, 0);
}

/* Writes a new config record (boot slot, device settings dev[]) to the older copy. */
static int write_cfg(int slot)
{
    int c;
    if (job.kind != J_NONE || eeprom_busy()) return PR_BUSY;
    if (!cfg_copies[0].writable || !cfg_copies[1].writable) return last_result = PR_ERR_PROTECTED;
    c = newest(cfg_copies);
    job.seq = c >= 0 ? cfg_copies[c].seq + 1 : 1;
    c = c < 0 ? 0 : c ^ 1;
    memset(buf, 0, CFG_SIZE);
    oc_put32(buf, MAGIC_CFG);
    oc_put32(buf + 4, job.seq);
    buf[8] = (uint8_t)slot;
    memcpy(buf + 9, dev, sizeof(dev));
    oc_put32(buf + 12, oc_crc32(buf, 12));
    cfg_copies[c].valid = 0;
    return start_job(J_CFG, SLOT_NONE, c, cfg_addr(c), CFG_SIZE, 1);
}

int profiles_set_boot(int slot)
{
    if (slot != SLOT_NONE && (slot < 0 || slot >= PROFILE_SLOTS)) return PR_ERR_ARG;
    return write_cfg(slot);
}

/* Device settings (sub level source, sub level, options): applied at once, even if they cannot be saved; the
 * source and the options change nothing while an EEPROM job runs (PR_BUSY); the sub level is always taken and
 * saved later. */
int profiles_set_dev(int i, int v)
{
    const uint8_t prev = dev[i];
    int r;
    if (i == DEV_SUB_SOURCE && (unsigned)v > 1u) return PR_ERR_ARG;
    if (i == DEV_SUB_LEVEL) {                          /* applied at once, saved 1 s after the last change */
        if (!SUB_LEVEL_SET(v)) return PR_ERR_ARG;
        dev[i] = (uint8_t)v;
        apply_dev();
        cfg_dirty = 1;
        cfg_due = g_audio.frames + 500u;
        return PR_OK;
    }
    dev[i] = (uint8_t)v;
    r = write_cfg(boot_slot);
    if (r == PR_BUSY) dev[i] = prev;
    else apply_dev();
    return r;
}

void profiles_modified(void)
{
    modified = 1;
    edits++;
}

void profiles_cycle(void)
{
    int i, start = active == SLOT_NONE ? PROFILE_SLOTS - 1 : active;
    for (i = 1; i <= PROFILE_SLOTS; i++) {
        int s = (start + i) % PROFILE_SLOTS;
        if (slots[s].valid) {
            profiles_load(s);
            return;
        }
    }
}

int profiles_busy(void) { return job.kind != J_NONE; }
int profiles_last_result(void) { return last_result; }
int profiles_active(void) { return active; }
int profiles_is_modified(void) { return modified; }
int profiles_boot_slot(void) { return boot_slot; }
const slot_info_t *profiles_slot(int slot) { return &slots[slot]; }
