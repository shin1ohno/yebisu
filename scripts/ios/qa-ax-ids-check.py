#!/usr/bin/env python3
"""Ties every QA accessibility identifier a Swift test uses to the app that marks it.

The QA accessibility layer (crates/yebisu/src/qa_ax.rs, UI test redesign plan 3.4) exposes
GPUI controls to XCTest by identifier. A test that names an identifier the app never marks
fails only on a Simulator, long after the change; this check runs where the sources are:

  * every identifier is a constant in crates/yebisu/src/qa_ax_ids.rs (value `qa.<...>`,
    unique), and no other app source (tests aside) spells a `"qa.` literal;
  * every `qa_ax::mark(...)` call in the app passes one of those constants, and every
    constant except STATE (published by the app root) is marked somewhere;
  * every `"qa.<id>"` string a Swift test uses (scripts/ios/*/Tests/*.swift) is one of the
    constants; an identifier composed at run time ("qa.p0.\\(x)") cannot be checked and is
    an error;
  * the QAAx*.swift helpers never decide by `isHittable` (S0: it ignores GPUI occlusion);
  * the build gate holds: the UIKit half is compiled only for the Simulator, the layer only
    for the Simulator and host tests, and `BUILD` is the Simulator cfg.

  qa-ax-ids-check.py [--root DIR]
"""
import argparse
from pathlib import Path
import re
import sys

SRC = "crates/yebisu/src"
IDS = SRC + "/qa_ax_ids.rs"
QA_AX = SRC + "/qa_ax.rs"
SWIFT_GLOB = "scripts/ios/*/Tests/*.swift"
NOT_MARKED = {"STATE"}  # the app root publishes it on every frame
SIM = 'all(target_os = "ios", target_abi = "sim")'
GATES = {
    "native": '#[cfg(%s)]\n#[path = "qa_ax_native.rs"]\nmod native;' % SIM,
    "layer": '#[cfg(any(%s, test))]\n#[path = "qa_ax_layer.rs"]\npub(crate) mod layer;' % SIM,
    "BUILD": "pub(crate) const BUILD: bool = cfg!(%s);" % SIM,
}

CONST = re.compile(r'pub\(crate\) const (\w+): &str = "([^"\\]*)";')
ID_TEXT = re.compile(r"qa\.[A-Za-z0-9_.-]*[A-Za-z0-9_-]")
MARK = re.compile(r"\bqa_ax::mark\(\s*([\w:]+)\s*,")
SWIFT_STRING = re.compile(r'"((?:[^"\\\n]|\\.)*)"')


def blank(match):
    return re.sub(r"[^\n]", " ", match.group(0))


def strip_comments(text):
    """Blank `/* */` and `//` comments outside string literals; line numbers are kept."""
    text = re.sub(r"/\*.*?\*/", blank, text, flags=re.S)
    out = []
    for line in text.splitlines():
        in_string, cut = False, len(line)
        for at, char in enumerate(line):
            if char == '"' and (at == 0 or line[at - 1] != "\\"):
                in_string = not in_string
            elif not in_string and line.startswith("//", at):
                cut = at
                break
        out.append(line[:cut])
    return "\n".join(out)


def line_of(text, at):
    return text.count("\n", 0, at) + 1


def rust_files(root):
    return sorted(path for path in (Path(root) / SRC).rglob("*.rs"))


def is_test_file(path):
    return path.stem.endswith("_tests") or "tests" in path.parts[:-1] or path.stem == "tests"


def defined_ids(root, problems):
    """name -> identifier from qa_ax_ids.rs."""
    path = Path(root) / IDS
    if not path.is_file():
        problems.append("%s is missing: it is the one list of QA identifiers" % IDS)
        return {}
    ids, values = {}, {}
    for match in CONST.finditer(strip_comments(path.read_text())):
        name, value = match.groups()
        if not re.fullmatch(ID_TEXT, value):
            problems.append("%s: %s = %r is not a QA identifier (qa.<area>.<control>)" % (IDS, name, value))
        if value in values:
            problems.append("%s: %s and %s both define %r" % (IDS, values[value], name, value))
        values[value] = name
        ids[name] = value
    return ids


def marks(root, ids, problems):
    """name -> ['file:line', ...] of the qa_ax::mark calls, and stray literals in app sources."""
    marked = {}
    for path in rust_files(root):
        rel = path.relative_to(root).as_posix()
        if is_test_file(path):
            continue
        text = strip_comments(path.read_text(errors="replace"))
        if rel != IDS:
            for match in re.finditer(r'"(%s)"' % ID_TEXT.pattern, text):
                problems.append("%s:%d spells %r: define it in %s and use the constant"
                                % (rel, line_of(text, match.start()), match.group(1), IDS))
        for match in MARK.finditer(text):
            argument = match.group(1)
            name = argument.split("::")[-1]
            where = "%s:%d" % (rel, line_of(text, match.start()))
            if not (argument.startswith(("ids::", "crate::qa_ax::ids::", "qa_ax::ids::")) and name in ids):
                problems.append("%s: qa_ax::mark(%s, ...) does not name a constant of %s" % (where, argument, IDS))
                continue
            marked.setdefault(name, []).append(where)
    for name in sorted(ids):
        if name not in NOT_MARKED and name not in marked:
            problems.append("%s (%r) is defined in %s but no qa_ax::mark call uses it: the app never exposes it"
                            % (name, ids[name], IDS))
    return marked


def swift_ids(root, problems):
    """identifier -> ['file:line', ...] of the strings Swift tests use."""
    used = {}
    for path in sorted(Path(root).glob(SWIFT_GLOB)):
        rel = path.relative_to(root).as_posix()
        text = strip_comments(path.read_text())
        if path.name.startswith("QAAx") and re.search(r"\bisHittable\b", text):
            problems.append("%s uses isHittable: confirm a press through qa.state instead (S0: isHittable "
                            "ignores GPUI occlusion)" % rel)
        for match in SWIFT_STRING.finditer(text):
            literal = match.group(1)
            head = re.match(r"qa\.[A-Za-z0-9_.-]*", literal)
            if not head:
                continue
            where = "%s:%d" % (rel, line_of(text, match.start()))
            rest = literal[head.end():]
            if rest.startswith("\\("):
                problems.append("%s: %r composes an identifier at run time; name a constant instead" % (where, literal))
            elif rest == "":
                used.setdefault(literal, []).append(where)
    return used


def gates(root, problems):
    path = Path(root) / QA_AX
    if not path.is_file():
        problems.append("%s is missing" % QA_AX)
        return
    text = path.read_text()
    for what, fragment in GATES.items():
        if fragment not in text:
            problems.append("%s no longer declares %s exactly as %r: the layer may reach a device or "
                            "non-QA build" % (QA_AX, what, fragment))
    for other in rust_files(root):
        rel = other.relative_to(root).as_posix()
        if rel != QA_AX and "qa_ax_native" in strip_comments(other.read_text(errors="replace")):
            problems.append("%s names qa_ax_native: only %s may declare it (Simulator only)" % (rel, QA_AX))


def check(root):
    """(rows, problems). rows are (identifier, constant, marked at, used at)."""
    problems = []
    ids = defined_ids(root, problems)
    marked = marks(root, ids, problems)
    used = swift_ids(root, problems)
    by_value = {value: name for name, value in ids.items()}
    for identifier in sorted(used):
        if identifier not in by_value:
            problems.append("Swift uses %r (%s) but %s does not define it: the app never exposes it"
                            % (identifier, "; ".join(used[identifier]), IDS))
    gates(root, problems)
    rows = [(value, name, ", ".join(marked.get(name, ["(app root)" if name in NOT_MARKED else "-"])),
             ", ".join(used.get(value, ["-"]))) for name, value in sorted(ids.items(), key=lambda item: item[1])]
    return rows, problems


def main(argv=None):
    here = Path(__file__).resolve()
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", default=str(here.parents[2]))
    args = parser.parse_args(argv)
    rows, problems = check(Path(args.root))
    for identifier, name, marked, used in rows:
        print("%-20s %-8s marked %s; used %s" % (identifier, name, marked, used))
    print("%d identifiers, %d used by Swift tests" % (len(rows), sum(1 for row in rows if row[3] != "-")))
    for problem in problems:
        print("ERROR", problem, file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
