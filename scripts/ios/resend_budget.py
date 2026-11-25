#!/usr/bin/env python3
"""The proven resends of one CI job, and their limit (issue #142 stage 4, Owner decision 5).

The UI tests send an input once more only when the app's own receipts prove it was lost, and
log each such resend as one `QA-RESEND {json}` line (QAAx.resend). Every hosted check script
appends one line per run to a ledger under target/ (`append`), also with zero resends; the
job's last step (`judge`, `if: always()`) totals the ledger and fails the job as
"Simulator input unstable" when there are more than LIMIT. A missing ledger fails too: no
check ran, so nothing proves the job's inputs.

    resend_budget.py judge target/sim-resends.jsonl
"""
import json
from pathlib import Path
import subprocess
import sys

LIMIT = 3
LEDGER = "target/sim-resends.jsonl"
MARK = "QA-RESEND "


def parse(text):
    """The resend records in xcodebuild output: the JSON after each `QA-RESEND ` mark."""
    found = []
    for line in (text or "").splitlines():
        at = line.find(MARK)
        if at < 0:
            continue
        try:
            record = json.loads(line[at + len(MARK):])
        except ValueError:
            record = {"kind": "unparsed", "line": line[at:][:200]}
        found.append(record if isinstance(record, dict) else {"kind": "unparsed", "line": line[at:][:200]})
    return found


def append(ledger, check, resends):
    """Adds the line of one check run (`check`, its resends) to the job's ledger."""
    ledger = Path(ledger)
    ledger.parent.mkdir(parents=True, exist_ok=True)
    with ledger.open("a") as file:
        file.write(json.dumps({"check": check, "count": len(resends), "resends": resends}, sort_keys=True) + "\n")


def record(check, output, ledger=None):
    """Appends `check`'s resends, parsed from its xcodebuild output, to the job ledger under the
    repository's target/ (also with none: the line shows the check ran). Returns the resends."""
    resends = parse(output)
    append(ledger or Path(__file__).resolve().parents[2] / LEDGER, check, resends)
    print(f"QA-RESEND ledger: {check} {len(resends)} proven resend(s)", flush=True)
    return resends


def run_tee(command, **kwargs):
    """subprocess.run(command, check=True) for a check that streams its output: prints each line as it
    comes and returns (returncode, the whole output) so the resend lines can be parsed after."""
    lines = []
    with subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, **kwargs) as process:
        for line in process.stdout:
            print(line, end="", flush=True)
            lines.append(line)
    return process.returncode, "".join(lines)


def judge(ledger, limit=LIMIT):
    """(exit code, message) for the job: 1 when the ledger is missing or holds more than `limit` resends."""
    ledger = Path(ledger)
    if not ledger.is_file():
        return 1, f"Simulator input unstable: no resend ledger at {ledger}; no check recorded its inputs"
    rows = [json.loads(line) for line in ledger.read_text().splitlines() if line.strip()]
    if not rows:
        return 1, f"Simulator input unstable: the resend ledger {ledger} is empty"
    total = sum(row.get("count", 0) for row in rows)
    listing = [f"{row['check']}: {r.get('kind')} {r.get('id', '')}".strip() for row in rows for r in row.get("resends", [])]
    if total > limit:
        return 1, f"Simulator input unstable: {total} proven resends in this job (limit {limit}): {listing}"
    return 0, f"Simulator input resends in this job: {total} (limit {limit}) from {len(rows)} check runs: {listing}"


def main(argv):
    if len(argv) != 3 or argv[1] != "judge":
        raise SystemExit("usage: resend_budget.py judge <ledger>")
    code, message = judge(argv[2])
    print(message, flush=True)
    return code


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
