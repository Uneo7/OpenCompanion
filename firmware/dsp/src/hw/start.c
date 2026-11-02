/* C part of the reset entry (reset.asm). The RTS start-up zeroed .bss and .far (zero_init records) and copied
 * the initialised data; with --ram_model the data is already in place, so only the zeroing is left. Zeroing here
 * keeps the firmware independent of whether the loader (boot1, RAM loader) fills these areas itself. */
#include <stdint.h>

/* .bss + .far, contiguous and 8-byte aligned and padded (lnk.cmd GROUP ZERO_INIT) */
extern far uint32_t __oc_zero_start[], __oc_zero_end[];

/* Stack anchor, as in the RTS boot.c: the linker only creates .stack (sized by -stack in lnk.cmd) when an input
 * section exists. Without it .stack would be empty and SP would point below the image. */
#pragma DATA_ALIGN(oc_stack, 8)
#pragma DATA_SECTION(oc_stack, ".stack")
far char oc_stack[8];

int main(void);
void oc_start(void);

void oc_start(void)
{
    uint32_t *p;
    for (p = __oc_zero_start; p < __oc_zero_end; p++)
        *p = 0;
    main();
    for (;;)
        ;
}
