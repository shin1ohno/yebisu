"""Collect the app's stderr copy into a check's artifact directory.

A QA Simulator build with YEBISU_SMOKE_OBSERVE or YEBISU_P2_OBSERVE copies its
stderr into Documents/app-stderr.log (crates/yebisu/src/qa_stderr.rs). An app
that XCTest launches has no other stderr anyone keeps, so without this a
`smoke observation dropped` line was lost (hosted p10, run 44406830068 job
135164241872). A check takes the offset before XCTest runs, collects the new
bytes into its directory, and fails on any line that says a record was lost.
"""
from pathlib import Path

NAME = "app-stderr.log"
# Lines the app prints when an observation record does not reach its file
# (crates/yebisu/src/observation.rs, crates/yebisu/src/app.rs).
LOSS = ("smoke observation dropped", "smoke observation writer", "yebisu QA stderr capture failed")


def offset(documents):
    """Size of the log before a run, 0 if the app never wrote one."""
    path = Path(documents) / NAME
    return path.stat().st_size if path.exists() else 0


def collect(documents, directory, start=0):
    """Copy the bytes written since start to directory/app-stderr.log; return them as text.

    Raises ValueError when there is no log, because then a lost record would be invisible.
    """
    path = Path(documents) / NAME
    if not path.exists():
        raise ValueError(f"app stderr was not captured: no Documents/{NAME}")
    data = path.read_bytes()[start:]
    Path(directory, NAME).write_bytes(data)
    return data.decode("utf-8", "replace")


def require_no_loss(text):
    """Raise ValueError naming the first lines that report a lost observation record."""
    lost = [line for line in text.splitlines() if line.startswith(LOSS)]
    if lost:
        raise ValueError(f"app lost observation records ({len(lost)} lines): " + " | ".join(lost[:3]))
