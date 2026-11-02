; Reset entry (replaces the RTS _c_int00 -> _auto_init_elf -> _args_main -> exit chain, ~2.3 KB of code).
; Same register set-up as the RTS boot.c: stack pointer, data page pointer, floating-point control registers.
; The image is linked with --ram_model (lnk.cmd): boot1 or the RAM loader writes initialised data in place, so
; the only run-time initialisation left is zeroing .bss and .far, done by oc_start() (start.c).

        .global _c_int00
        .global __oc_stack_end
        .global __TI_STATIC_BASE
        .ref    oc_start

        .sect   ".text"
_c_int00:
        MVKL    .S2     __oc_stack_end - 4,B15
        MVKH    .S2     __oc_stack_end - 4,B15
        AND     .S2     -8,B15,B15              ; SP 8-byte aligned
        MVKL    .S2     __TI_STATIC_BASE,B14
        MVKH    .S2     __TI_STATIC_BASE,B14
        ZERO    .L2     B4
        MVC     .S2     B4,FADCR
        MVC     .S2     B4,FMCR
; Program cache on: CSR.PCC (bits 7:5) = 010b. The ROM boot loader may leave it in bypass (the Bose boot
; driver switches PCC to 100b while it loads sections, 0x10002024, and back to 010b, 0x10001DD0); nothing on
; our boot chain (boot0, boot1, rescue, RAM loader) sets it back, so every fetch went to RAM (~3x slower).
; The cache holds no stale line: boot1 / the RAM loader invalidate L1P before the jump, and a bypassed cache
; is not filled.
        MVC     .S2     CSR,B4
        CLR     .S2     B4,5,7,B4
        SET     .S2     B4,6,6,B4
        MVC     .S2     B4,CSR
        B       .S2     oc_start                ; never returns
        NOP     5
