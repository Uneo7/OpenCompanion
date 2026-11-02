#!/bin/sh
# Host tests of the boot chain (no hardware involved). Run from firmware/dsp/boot after `make`.
#   ZIG=path/to/zig.exe CARGO=cargo sh test/run_host_tests.sh
set -e
ZIG=${ZIG:-zig}
CARGO=${CARGO:-cargo}
APP_ELF=${APP_ELF:-../build/opencompanion_dsp.out}
OUT=build/test
mkdir -p $OUT
(cd ../../../app && $CARGO build -q -p oc-ais --bin oc-bootimg)
BOOTIMG=../../../app/target/debug/oc-bootimg
# --allow-legacy-app: the current firmware does not declare the new layout yet (test only, never flashed).
$BOOTIMG build --tramp build/tramp.out --boot1 build/boot1.out --app $APP_ELF --allow-legacy-app --seq 1 -o $OUT/ee.bin
$BOOTIMG build --tramp build/tramp.out --boot1 build/boot1.out --app $APP_ELF --allow-legacy-app --seq 2 --trial -o $OUT/ee_trial.bin
$ZIG cc -target x86-windows-gnu -O1 -Wall -Wextra -Werror -Iboot1/include -o $OUT/host_image.exe test/host_image.c \
    boot1/src/image.c boot1/src/stream.c boot1/src/lz4.c boot1/src/crc32.c boot1/src/trial.c boot1/src/util.c
$OUT/host_image.exe $OUT/ee.bin $OUT/ram.bin
py -3 -I test/check_ram.py $APP_ELF $OUT/ram.bin
$OUT/host_image.exe $OUT/ee.bin --mutate
$OUT/host_image.exe $OUT/ee_trial.bin --trial
echo "host tests: OK"
