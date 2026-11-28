"""Every Simulator job uploads the guard's log, even after a failed or cancelled step.

scripts/guarded.py writes its disk, delay, load, swap and memory probes to target/disk.log.
Issue #256 judges a cancelled job by whether a VM-wide stall in those probes came first, so the
log must reach the job's evidence artifact whichever way the job ended.
"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
CI = (ROOT / ".github/workflows/ci.yml").read_text()
SIMULATOR_JOBS = (
    ("edit-menu", "edit-menu-evidence"),
    ("simulator-smoke", "simulator-smoke-evidence"),
    ("layout-smoke", "layout-smoke-evidence"),
)


def job(name):
    match = re.search(rf"^  {name}:\n(.*?)(?=^  [a-z][a-z-]*:\n|\Z)", CI, re.S | re.M)
    if match is None:
        raise AssertionError(f"job {name} not found")
    return match.group(1)


def upload_step(body, artifact):
    for step in re.split(r"^      - ", body, flags=re.M):
        if f"          name: {artifact}\n" in step:
            return step
    raise AssertionError(f"no upload step names {artifact}")


class GuardLogTests(unittest.TestCase):
    def test_the_guard_writes_target_disk_log(self):
        guarded = (ROOT / "scripts/guarded.py").read_text()
        self.assertIn('ROOT = Path(__file__).resolve().parent.parent / "target"\n', guarded)
        self.assertIn("(ROOT / 'disk.log').open('a')", guarded)

    def test_every_simulator_job_uploads_the_guard_log_whatever_the_outcome(self):
        for name, artifact in SIMULATOR_JOBS:
            with self.subTest(job=name):
                body = job(name)
                self.assertIn("just guarded ", body)
                step = upload_step(body, artifact)
                self.assertIn("        if: always()\n", step)
                self.assertIn("        uses: actions/upload-artifact@v4\n", step)
                self.assertIn("            target/disk.log\n", step)


if __name__ == "__main__":
    unittest.main()
