"""Keep the crash reports written while a Simulator suite ran (issue #261).

A process that crashes inside the iOS Simulator, SpringBoard included, leaves its
report in the host's ~/Library/Logs/DiagnosticReports (the report's header says
"platform":7). The hosted runner discards that directory with the VM, so the
suite copies every report newer than its own start into the uploaded evidence.
"""
from pathlib import Path
import shutil

REPORTS = Path.home() / "Library" / "Logs" / "DiagnosticReports"


def copy_since(since, destination, source=REPORTS):
    """Copy each .ips report in source modified at or after `since` (epoch seconds)
    into destination and return the copied names. A missing source copies nothing;
    a report that vanishes or cannot be read is named in the result as skipped."""
    copied = []
    try:
        reports = sorted(source.glob("*.ips"))
    except OSError:
        return copied
    for report in reports:
        try:
            if report.stat().st_mtime < since:
                continue
            destination.mkdir(parents=True, exist_ok=True)
            shutil.copy2(report, destination / report.name)
            copied.append(report.name)
        except OSError as error:
            copied.append(f"{report.name} (skipped: {error.strerror or error})")
    return copied
