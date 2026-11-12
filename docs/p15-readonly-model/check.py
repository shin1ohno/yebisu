#!/usr/bin/env python3
"""Bounded source/fixture checks; explicitly NOT execution of the Rust model."""
import json
from pathlib import Path
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[2]
DOC = Path(__file__).resolve().parent
SRC = ROOT / "crates/yebisu/src"
BASE = "dc2511fe695935cdce60ba6bb3cfb4288a425778"


def request_is_read_only(source):
    match = re.search(r"pub\(crate\) enum Request \{(.*?)\n\}", source, re.S)
    if match is None:
        return False
    body = match.group(1).strip()
    return body == "Read { identity: Identity },"


def check():
    model = (SRC / "host_settings/mod.rs").read_text()
    assert request_is_read_only(model), "read-only request contract"
    mutated = model.replace("Read { identity: Identity },", "Read { identity: Identity },\n    Save,")
    assert mutated != model and not request_is_read_only(mutated), "negative write mutation"
    fixtures = json.loads((DOC / "fixtures.json").read_text())
    for fixture in fixtures["toml"]:
        try:
            document = tomllib.loads(fixture["text"])
            table = document.get("experimental", {})
            if not isinstance(table, dict):
                actual = "InvalidField"
            else:
                actual = table.get("pane_history")
                if actual is not None and not isinstance(actual, bool):
                    actual = "InvalidField"
        except tomllib.TOMLDecodeError:
            actual = "InvalidToml"
        assert actual == fixture.get("error", fixture.get("expected")), fixture["name"]
    # Positive control: retain the exact owner-default implementation across integration.
    relative = "crates/yebisu/src/theme.rs"
    baseline = subprocess.check_output(["git", "-C", str(ROOT), "show", f"{BASE}:{relative}"]).decode()
    marker = "impl Default for Theme"
    assert marker in baseline and marker in (ROOT / relative).read_text()
    assert baseline.split(marker, 1)[1] == (ROOT / relative).read_text().split(marker, 1)[1]
    main = (SRC / "main.rs").read_text()
    assert "mod theme_picker;" in main and 'host_settings/availability.rs' in main
    assert "Page::Appearance => self.render_p15_settings(cx)" in (SRC / "settings_sheet/view.rs").read_text()
    for directory in [SRC / "host_settings", SRC / "theme_picker"]:
        for file in directory.rglob("*.rs"):
            assert len(file.read_text().splitlines()) <= 1000, file
    print(json.dumps({
        "result":"pass", "scope":"Python source/fixture validation only; Rust NOT run",
        "toml_fixtures":len(fixtures["toml"]),
        "request_positive_control":True, "source_write_mutation_rejected":True,
        "preserved_default_palette":True, "mounted_route_source":True,
        "adapter_negatives_unverified":len(fixtures["adapter_negatives_unverified"]),
        "network_receipts":False, "native_acceptance":False,
    }, indent=2))


if __name__ == "__main__":
    check()
