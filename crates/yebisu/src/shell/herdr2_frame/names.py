"""Name each folder the official app's way: by its git checkout's top.

FOLDERS is set above this line by yebisu: the folders, NUL-separated and
hex-encoded, so document text never reaches the shell. The output is the
home folder, then each folder and its checkout top (the folder itself
outside git), every field ending in NUL.
"""
import os
import subprocess
import sys

out = sys.stdout.buffer
out.write(os.fsencode(os.path.realpath(os.path.expanduser("~"))) + b"\0")
for folder in bytes.fromhex(FOLDERS).split(b"\0"):
    if not folder:
        continue
    top = folder
    try:
        found = subprocess.run(
            ["git", "-C", folder, "rev-parse", "--show-toplevel"],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            timeout=0.5,
            check=False,
        )
        if found.returncode == 0 and found.stdout.strip():
            top = found.stdout.rstrip(b"\n")
    except (OSError, subprocess.SubprocessError):
        pass
    out.write(folder + b"\0" + top + b"\0")
out.flush()
