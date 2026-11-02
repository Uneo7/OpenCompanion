"""Transient check of a profile switch (test/host_profile_swap.c output files).
    python profile_swap_check.py swap.bin glide.bin FREQ_HZ
Click detector as eq_glide_check.py: the largest second difference after the switch, divided by A w^2 with
A = the larger steady amplitude (before / after) and w the tone frequency. 1 = no corner; bound kept by the
firmware (swap path): <= 10, as for the EQ glide. Also prints the peak during the switch / larger steady peak."""
import sys
import numpy as np

B = 96
w = 2 * np.pi * float(sys.argv[3]) / 48000
for path in sys.argv[1:3]:
    y = np.fromfile(path, np.float32).astype(float)
    a = max(np.abs(y[80 * B:100 * B]).max(), np.abs(y[-20 * B:]).max())
    d2 = np.abs(np.diff(y, 2))[100 * B:]
    print("%s: corner %.1f, peak %.2f" % (path, d2.max() / (a * w * w), np.abs(y[100 * B:]).max() / a))
