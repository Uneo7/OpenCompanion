# The DA705 core is a C67x+. Only TI C6000 CGT 7.4.x (and earlier) can target it (-mv67p).
# CGT 8.3 dropped C67x/C67x+: with it we can only build for C674x (-mv6740), which may emit C64x+-only
# instructions (compact 16-bit, SPLOOP...) that the DA705 cannot execute. That build is a COMPILE CHECK
# ONLY and must never be loaded on the speaker.
#
# Usage: make                      -> build/opencompanion_dsp.out (uses CGT 7.4.x if installed)
#        make CHECK_ONLY=1         -> compile check with CGT 8.3 (not runnable)
#        make STAGE=1|2|3|4        -> bring-up stage (see src/main.c), default 4
#        make clean

ROOT      := ../../..
CGT74     ?= $(ROOT)/ti-cgt-c6000_7.4.24
CGT83     ?= $(ROOT)/ti-cgt-c6000_8.3.7

ifeq ($(CHECK_ONLY),1)
  CGT     := $(CGT83)
  SILICON := -mv6740 --no_compress
  RTSLIB  := rts6740_elf.lib
  CSTD    := --c99
else
  CGT     := $(CGT74)
  SILICON := -mv67p
  # Not shipped prebuilt with CGT 7.4.24; build it once with:
  #   cd ti-cgt-c6000_7.4.24/lib && ./mklib --pattern=rts67plus_elf.lib --index=libc.a --install_to=. --compiler_bin_dir=../bin
  RTSLIB  := rts67plus_elf.lib
  CSTD    :=
  ifeq ($(wildcard $(CGT74)/bin/cl6x*)$(filter clean,$(MAKECMDGOALS)),)
    $(error CGT 7.4.x not found in $(CGT74). Install TI C6000 CGT 7.4.24 there, or run "make CHECK_ONLY=1")
  endif
endif

CC      := $(CGT)/bin/cl6x
BUILD   := build
# make PROF=1: per-stage CPU profile (include/prof.h, console PRF0 / PRF1), output in build_prof/. For RAM
# loading only: such an image is never written to the EEPROM (it may exceed the body area).
PROF    ?= 0
ifeq ($(PROF),1)
  BUILD   := build_prof
  PROFDEF := -DOC_PROF
endif
TARGET  := $(BUILD)/opencompanion_dsp.out

STAGE   ?= 4
CFLAGS  := $(SILICON) --abi=eabi -O2 $(CSTD) -DOC_STAGE=$(STAGE) $(PROFDEF) --mem_model:data=far_aggregates --display_error_number --diag_warning=225 \
           -I$(CGT)/include -Iinclude
# Firmware version, reported by SVM and written into the image header by the host installer (oc-device reads
# it back from the ELF): FW_VERSION = semantic version, FW_BUILD = UTC build stamp, so that two builds never
# compare equal. console.c, which prints it, is rebuilt every time.
FW_VERSION ?= 0.2.7
FW_BUILD   ?= $(shell date -u +%Y%m%d%H%M%S)
OC_VERSION_STR := OC$(FW_VERSION)$(if $(strip $(FW_BUILD)),+$(strip $(FW_BUILD)))
CFLAGS  += -DOC_VERSION='"$(OC_VERSION_STR)"'
LDFLAGS := --ram_model -i$(CGT)/lib --reread_libs --warn_sections
# The RTS comes after the objects: our reset entry (_c_int00) and memcpy must win over the library members.
LDLIBS  := -l$(RTSLIB)


# Sources: src/main.c and one sub-folder per area (README "Architecture"); object names are unique.
SRC_DIRS := src src/dsp src/engine src/hw src/audio src/bose src/protect src/control src/storage src/console src/util
SRCS_C   := $(foreach d,$(SRC_DIRS),$(wildcard $(d)/*.c))
SRCS_ASM := $(foreach d,$(SRC_DIRS),$(wildcard $(d)/*.asm))
OBJS     := $(patsubst %,$(BUILD)/%.obj,$(basename $(notdir $(SRCS_C) $(SRCS_ASM))))
vpath %.c   $(SRC_DIRS)
vpath %.asm $(SRC_DIRS)

# Code size: the image body must fit the 48 KB EEPROM area once LZ4-compressed (boot/docs/LAYOUT.md).
# Everything is built with --opt_for_space=3 except SPEED_SRCS: the hot block kernels (src/dsp/k_vec.c gain /
# multiply-add / peak + power, k_biquad.c the 4-lane biquad section), which carry most of the audio CPU load and
# need software pipelining (README "CPU"). The lighter loops (k_dyn.c, k_ctl.c) are size-built: EEPROM space.
SPEED_SRCS := k_vec k_biquad k_dyn
SIZE_SRCS  := $(filter-out $(SPEED_SRCS),$(basename $(notdir $(SRCS_C))))
SIZE_OBJS  := $(patsubst %,$(BUILD)/%.obj,$(SIZE_SRCS))
$(SIZE_OBJS): CFLAGS += --opt_for_space=3
# --opt_for_space=1 still software-pipelines them (same ii as -ms0, smaller); -ms2 and up disable pipelining.
$(patsubst %,$(BUILD)/%.obj,$(SPEED_SRCS)): CFLAGS += --opt_for_space=1
# CTRL_SRCS: control-plane and init code (start-up, console, storage, profile handling, one-time hardware
# set-up; no per-sample loop) also gets -O1, smaller than -O2 here (~220 bytes of body). Also -O1: the ramp
# converter and the TAS link (per-frame / per-byte work, no sample loop) and the EQ designer (on changes only).
CTRL_SRCS := main start pll dmax dmax_ucode mcasp1 mcasp0_gpio ramp i2c_slave eq_design \
             handoff params profile_bose spk_mode volume volume_curve \
             bootctl crc32 eeprom profiles bose_tables \
             console console_dev console_inputs console_params console_util libc_min \
             sub_level
CTRL_OBJS := $(patsubst %,$(BUILD)/%.obj,$(CTRL_SRCS))
$(CTRL_OBJS): CFLAGS += -O1

all: $(TARGET)

$(BUILD):
	mkdir -p $(BUILD)

$(BUILD)/%.obj: %.c $(wildcard include/*.h) | $(BUILD)
	$(CC) $(CFLAGS) --obj_directory=$(BUILD) -c $<

$(BUILD)/%.obj: %.asm | $(BUILD)
	$(CC) $(CFLAGS) --obj_directory=$(BUILD) -c $<

$(TARGET): $(OBJS) lnk.cmd
	$(CC) $(CFLAGS) -z $(LDFLAGS) -m $(BUILD)/opencompanion_dsp.map -o $@ $(OBJS) lnk.cmd $(LDLIBS)

clean:
	rm -rf $(BUILD)

.PHONY: all clean FORCE
FORCE:
$(BUILD)/console.obj: FORCE
