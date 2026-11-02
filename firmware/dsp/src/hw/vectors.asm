; Interrupt service table (IST): 16 entries of 32 bytes, 1 KB aligned (C674x CPU reference).
; Each entry saves B0, loads the handler address, branches and restores B0 in the delay slots.

        .global vectors
        .ref    _c_int00
        .ref    isr_nmi
        .ref    isr_dmax_tcc
        .ref    isr_i2c
        .ref    isr_mcasp0_frame

VEC     .macro  handler
        STW     .D2T2   B0,*B15--[2]
||      MVKL    .S2     handler,B0
        MVKH    .S2     handler,B0
        B       .S2     B0
        LDW     .D2T2   *++B15[2],B0
        NOP     4
        NOP
        NOP
        .endm

UNUSED  .macro
        B       .S2     IRP
        NOP     5
        NOP
        NOP
        NOP
        NOP
        NOP
        NOP
        .endm

        .sect   ".vecs"
        .nocmp                  ; no 16-bit compact instructions: fixed 32-byte entries (and C67x+ has none)
        .align  1024
vectors:
v_reset: VEC    _c_int00        ; 0  reset (not used when booted by AIS, kept for completeness)
v_nmi:   VEC    isr_nmi         ; 1  NMI
v_rsv2:  UNUSED                 ; 2
v_rsv3:  UNUSED                 ; 3
v_int4:  UNUSED                 ; 4  RTI 0
v_int5:  UNUSED                 ; 5  RTI 1..3
v_int6:  UNUSED                 ; 6  UHPI
v_int7:  UNUSED                 ; 7  dMAX FIFO status
v_int8:  VEC    isr_dmax_tcc    ; 8  dMAX transfer completion
v_int9:  VEC    isr_mcasp0_frame ; 9  dMAX interrupt event 26: McASP0 start of frame (ramp converter)
v_int10: UNUSED                 ; 10
v_int11: UNUSED                 ; 11
v_int12: UNUSED                 ; 12
v_int13: UNUSED                 ; 13
v_int14: VEC    isr_i2c         ; 14 I2C0/I2C1/SPI
v_int15: UNUSED                 ; 15
