"""Transient check of the EQ coefficient glide (test/host_eq_glide.c output files).
    python eq_glide_check.py glide.bin abrupt.bin old.bin
Click detector: the largest second difference |y[n] - 2 y[n-1] + y[n-2]| around each change, divided by its
value for the steady 50 Hz tone (A w^2, A = steady amplitude after the change). 1 = no corner at all; a click
or a state jump gives tens to thousands. Bound kept by the firmware (glide): <= 10 for both changes."""
import sys
import numpy as np

B, W = 96, 2 * np.pi * 50 / 48000
for path in sys.argv[1:]:
    y = np.fromfile(path, np.float32).astype(float)
    d2 = np.abs(np.diff(y, 2))
    out = []
    for f0, f1 in ((100, 200), (200, 400)):
        a = np.abs(y[(f1 - 20) * B:f1 * B]).max()
        out.append("frames %d..: %.1f" % (f0, d2[f0 * B - 2:f1 * B - 2].max() / (a * W * W)))
    print(path, " ".join(out))
