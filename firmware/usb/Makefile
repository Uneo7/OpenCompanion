# OpenCompanion TAS1020B firmwares (SDCC, 8051).
#
#   make                 -> both targets
#   make oc_tas          -> build/oc_tas/oc_tas.{bin,dfu}            OpenCompanion firmware (app/)
#   make oc_tas_test     -> build/oc_tas_test/oc_tas_test.{bin,dfu}  ROM-dump test firmware (test/)
#   make size            -> code size and memory usage of oc_tas
#   make clean
#
# SDCC must be in PATH (Windows default install: C:\Program Files\SDCC\bin).
# PYTHON must be Python 3 (on Windows: make PYTHON="py -3").
# Both images are for RAM download through the ROM DFU: header Attributes bit 1 = 0 (scripts/mkdfu.py).

SDCC    ?= sdcc
SDAS    ?= sdas8051
MAKEBIN ?= makebin
ifeq ($(OS),Windows_NT)
  PYTHON ?= py -3
else
  PYTHON ?= python3
endif

BUILD   := build
CFLAGS  := -mmcs51 --model-small --std-c11 --opt-code-size --peep-return --no-xinit-opt -Icommon
# Code runs from the 6016-byte program RAM (0x0000..0x177F, DM SLES025B 6.1).
LDBASE  := -mmcs51 --model-small --code-loc 0x0000 --code-size 0x1780 --iram-size 256

# --- oc_tas_test: no XDATA variables (endpoint buffers only) ---
TEST_SRCS := test/main.c common/usb_core.c common/i2c.c common/xreg.c test/usb_app.c test/descriptors.c test/commands.c test/restore_header.c test/board.c
TEST_DIR  := $(BUILD)/oc_tas_test
TEST_RELS := $(patsubst %.c,$(TEST_DIR)/%.rel,$(notdir $(TEST_SRCS)))
TEST_INC  := -Itest

# --- oc_tas: XDATA variables in the last 96 bytes of the endpoint buffer space (app/config.h) ---
# app/sys/crt0.asm (reset/interrupt vectors, C start-up) is linked first; the C entry point is oc_main().
APP_SRCS := app/main.c common/usb_core.c common/xreg.c \
            common/i2c.c app/sys/tick.c app/sys/rom.c \
            app/usb/usb_app.c app/usb/uac.c app/usb/hid.c \
            app/usb/uac_notify.c app/audio/stream.c app/audio/servo.c \
            app/board/power.c app/board/dsp_link.c app/board/pod.c app/board/pod_sync.c \
            app/proto/protocol.c app/proto/eeprom.c app/data/ee_area.c
APP_DIR  := $(BUILD)/oc_tas
APP_RELS := $(APP_DIR)/crt0.rel $(patsubst %.c,$(APP_DIR)/%.rel,$(notdir $(APP_SRCS)))
APP_INC  := -Iapp -Iapp/sys -Iapp/usb -Iapp/audio -Iapp/board -Iapp/proto -Iapp/data
APP_LD   := --xram-loc 0xFEC8 --xram-size 96

all: oc_tas oc_tas_test

oc_tas: $(APP_DIR)/oc_tas.dfu $(APP_DIR)/oc_tas_data.bin
# Also copied to build/oc_tas_test.dfu, the default image of tools/tas_dfu_run.ps1.
oc_tas_test: $(BUILD)/oc_tas_test.dfu
$(BUILD)/oc_tas_test.dfu: $(TEST_DIR)/oc_tas_test.dfu
	cp $< $@

# oc_tas_test: main.c must come first in the link so that its module provides the reset vector and startup code.
# oc_tas: crt0.rel comes first (app/sys/crt0.asm).
# Explicit source -> object pairs: both targets have files with the same names (main.c, usb_app.c...).
TEST_HDRS := $(wildcard common/*.h test/*.h)
define test_rule
$(TEST_DIR)/$(notdir $(1:.c=.rel)): $(1) $(TEST_HDRS)
	@mkdir -p $$(@D)
	$$(SDCC) $$(CFLAGS) $$(TEST_INC) -c $$< -o $$@
endef
$(foreach s,$(TEST_SRCS),$(eval $(call test_rule,$(s))))

$(TEST_DIR)/oc_tas_test.ihx: $(TEST_RELS)
	$(SDCC) $(LDBASE) --xram-size 0 $(TEST_RELS) -o $@

APP_HDRS := $(wildcard common/*.h app/*.h app/*/*.h)
define app_rule
$(APP_DIR)/$(notdir $(1:.c=.rel)): $(1) $(APP_HDRS)
	@mkdir -p $$(@D)
	$$(SDCC) $$(CFLAGS) $$(APP_INC) -c $$< -o $$@
endef
$(foreach s,$(APP_SRCS),$(eval $(call app_rule,$(s))))

$(APP_DIR)/crt0.rel: app/sys/crt0.asm
	@mkdir -p $(@D)
	$(SDAS) -plosgff $@ $<

$(APP_DIR)/oc_tas.ihx: $(APP_RELS)
	$(SDCC) $(LDBASE) $(APP_LD) $(APP_RELS) -o $@

# U6002 data area (app/data/ee_layout.h): app/data/ee_data.c on its own (never linked into the firmware), then
# header + CRC-32 (scripts/mkeedata.py). Written at 0x1800 by the host tools (oc tas data write, oc install tas).
DATA_DIR := $(APP_DIR)/data
$(DATA_DIR)/ee_data.rel: app/data/ee_data.c $(APP_HDRS)
	@mkdir -p $(@D)
	$(SDCC) $(CFLAGS) $(APP_INC) -c $< -o $@

$(DATA_DIR)/ee_data.ihx: $(DATA_DIR)/ee_data.rel
	$(SDCC) -mmcs51 --model-small --code-loc 0x0000 --code-size 0x0800 $< -o $@

$(APP_DIR)/oc_tas_data.bin: $(DATA_DIR)/ee_data.bin scripts/mkeedata.py app/data/ee_layout.h
	$(PYTHON) scripts/mkeedata.py $(DATA_DIR)/ee_data app/data/ee_layout.h $@

%.bin: %.ihx
	$(MAKEBIN) -p $< $@

# oc_tas: header firmware-version byte = minor x 16 + patch of app/config.h (0.4.0 -> 0x40, 0.4.10 -> 0x4A), seen by the installer's
# update detection (oc-install tas/target.rs); oc_tas_test keeps the mkdfu.py default (0xF0).
FW_HDR := $(shell awk '$$2 == "FW_VERSION_MINOR" { m = $$3 } $$2 == "FW_VERSION_PATCH" { p = $$3 } END { printf "0x%02x", m * 16 + p }' app/config.h)
$(APP_DIR)/oc_tas.dfu: DFU_ARGS := --firmware-version $(FW_HDR)
$(APP_DIR)/oc_tas.dfu: app/config.h

%.dfu: %.bin scripts/mkdfu.py
	$(PYTHON) scripts/mkdfu.py $(DFU_ARGS) $< $@

size: $(APP_DIR)/oc_tas.bin
	$(PYTHON) scripts/size_report.py $(APP_DIR)/oc_tas

clean:
	rm -rf $(BUILD)

.PHONY: all oc_tas oc_tas_test size clean
.SECONDARY:
