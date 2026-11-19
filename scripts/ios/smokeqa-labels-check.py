#!/usr/bin/env python3
"""Ties every string SmokeQA taps or OCR-matches to the app that shows it.

SmokeQA finds GPUI controls by reading the screen with Vision, so a control whose
text changes in the app, or a test that names a text the app never paints, fails
only on a Simulator run, long after the change. This check runs where the sources
are, in the check job:

  * every string passed to tapText, tapTextBox, tapMenuItem, textVisible or waitForRenderedText,
    every string the P15 host screen is matched against (text.contains), and the
    HOSTS header scrollBoard reads, is collected from Tests/SmokeQA*.swift;
  * each must have an entry in SOURCES (the app paints it as a literal) or in
    ALLOWED (the app composes it, or the test types it, with the reason), and every
    location of the entry must still contain its fragment;
  * an entry no test uses any more is an error, so neither table grows stale.

A call whose label the scanner cannot resolve to literals is an error, never skipped,
so a new indirect label cannot slip through. A PR that moves or renames a control
text edits its entry here, which is the point: the reviewer sees the label move.

  smokeqa-labels-check.py [--root DIR] [--tests DIR]
"""
import argparse
from pathlib import Path
import re
import sys

LABEL_CALLS = ("tapText", "tapTextBox", "tapMenuItem", "textVisible", "waitForRenderedText")
# `.contains("x")` receivers that are OCR output, and the ones that are not.
OCR_RECEIVERS = ("text", "string")
NOT_OCR_RECEIVERS = ("name",)  # setUpWithError: name.contains("testP9Core")

SRC = "crates/yebisu/src/"
HANDOFF = SRC + "handoff_view.rs"
SETTINGS_VIEW = SRC + "settings_sheet/view.rs"
P15_VIEW = SRC + "p15_settings_view.rs"
EDIT_MENU_QA = SRC + "edit_menu_qa.rs"
OVERFLOW_MENU = SRC + "shell/overflow_menu.rs"  # L2a ... menu
SETTINGS_SECTIONS = SRC + "settings_pages/sections.rs"
SETTINGS_CHOICES = SRC + "settings_pages/choices.rs"

# label -> locations (file, fragment) where the app paints that text as a literal.
# Every location must still contain its fragment. Sorted by label.
# keep-sorted start
SOURCES = {
    "Agent needs attention": [(SRC + "notifications.rs", '"Agent needs attention"')],
    "Close": [(SETTINGS_VIEW, 'CLOSE_LABEL: &str = "Close"')],
    "Double tap": [(SETTINGS_CHOICES, '["Double tap", "Drag"]')],
    "Discard composition": [(SRC + "sh1_156_qa.rs", '"Discard composition"')],
    "File path unavailable": [(P15_VIEW, '"File path unavailable"')],
    "HOSTS": [(SRC + "shell/sidebar/view.rs", '"HOSTS"')],
    "High contrast": [(SRC + "theme_picker/mod.rs", '"High contrast"')],
    "Input": [(SETTINGS_SECTIONS, 'Self::Input => "Input"')],
    "Larger": [(SETTINGS_VIEW, '"font-plus", "Larger"')],
    "Menu execute": [(EDIT_MENU_QA, '"Menu execute"')],
    "Menu prepare": [(EDIT_MENU_QA, '"Menu prepare"')],
    "Menu select": [(EDIT_MENU_QA, '"Menu select"')],
    "Native matrix collected": [(SRC + "native_observer_qa.rs", '"Native matrix collected"')],
    "Native next": [(SRC + "native_edit_qa.rs", '"Native next"'), (SRC + "native_observer_qa.rs", '"Native next"')],
    "Observer matrix collected": [(SRC + "settings_sheet/observer_qa.rs", '"Observer matrix collected"')],
    "Observer next": [(SRC + "settings_sheet/observer_qa.rs", '"Observer next"')],
    "On selection": [(SETTINGS_CHOICES, '["Manual", "On selection"]')],
    "Owner default": [(SRC + "theme_picker/mod.rs", '"Owner default"')],
    "P14 next": [(SRC + "notification_view/qa.rs", '"P14 next"')],
    "Probe modal callbacks": [(SRC + "settings_sheet/qa.rs", '"Probe modal callbacks"')],
    "Select text": [(OVERFLOW_MENU, 'Self::SelectText => "Select text"')],
    "Settings": [(SRC + "shell/footer.rs", 'SETTINGS: &str = "Settings"')],  # L2b sidebar footer
    "Theme": [(SRC + "settings_pages/sections.rs", 'Self::Theme => "Theme"')],  # L1 canvas section title
    "Type here": [(SRC + "mock_peer.rs", "Type here")],
    "Typing is paused": [(SRC + "shell/edit_bar.rs", 'PAUSED_TYPING_STATUS: &str = "Typing is paused')],  # status bar while selecting
    "daemon-effective state unverified": [(P15_VIEW, "daemon-effective state unverified")],
    "qa-alpha": [(SRC + "app.rs", '"qa-alpha"')],
    "qa-beta": [(SRC + "app.rs", '"qa-beta"')],
}
# keep-sorted end

# label -> (reason, locations). The app does not paint these as one literal: it composes
# them, or the test types them. The locations are what composes or shows them; if one
# disappears the reason no longer holds and the entry must be decided again.
# keep-sorted start
ALLOWED = {
    "Async counter": ("P0 counter row, painted as format!(\"Async counter: {}\", ticks)",
                      [(SRC + "app.rs", 'format!("Async counter: {}"')]),
    "Host: ": ("prefix of the host line, composed from the selected host name",
               [(P15_VIEW, 'format!("Host: {target}")')]),
    "QuietHours": ("status line is \"Notifications: {error:?}\", so the text is the Failure variant name",
                   [(SRC + "notification_view.rs", 'format!("Notifications: {error:?}")'), (SRC + "notifications.rs", "QuietHours,")]),
    "Unavailable": ("start of the read-only host settings message for a connection that cannot authorize a read",
                    [(SRC + "host_settings/availability.rs", '"Unavailable \u2014 this connection cannot authorize')]),
    "idle": ("typed by the test itself (typeText) into the P0 input row, which paints the typed text",
             [(SRC + "app.rs", ".child(self.input.text.clone())")]),
    "input": ("typed by the test itself (typeText) into the P0 input row, which paints the typed text",
              [(SRC + "app.rs", ".child(self.input.text.clone())")]),
    "returned": ("typed by the test itself (typeText) into the P0 input row, which paints the typed text",
                 [(SRC + "app.rs", ".child(self.input.text.clone())")]),
}
# keep-sorted end


def strip_comments(text):
    """Blank `//` comments, keeping string literals (Swift strings here have no escapes of `//`)."""
    out = []
    for line in text.splitlines():
        in_string = False
        cut = len(line)
        for at, char in enumerate(line):
            if char == '"' and (at == 0 or line[at - 1] != "\\"):
                in_string = not in_string
            elif not in_string and line.startswith("//", at):
                cut = at
                break
        out.append(line[:cut])
    return "\n".join(out)


def strip_rust_comments(text):
    """Blank `//` and `/* */` comments so a commented-out control no longer counts as painted; line numbers are kept."""
    text = re.sub(r"/\*.*?\*/", lambda m: re.sub(r"[^\n]", " ", m.group(0)), text, flags=re.S)
    return strip_comments(text)


def first_argument(text, start):
    """The text of the first argument of the call whose `(` is at text[start - 1]."""
    depth, in_string = 0, False
    for at in range(start, len(text)):
        char = text[at]
        if in_string:
            if char == '"' and text[at - 1] != "\\":
                in_string = False
            continue
        if char == '"':
            in_string = True
        elif char in "([{":
            depth += 1
        elif char in ")]}":
            if depth == 0:
                return text[start:at].strip()
            depth -= 1
        elif char == "," and depth == 0:
            return text[start:at].strip()
    raise ValueError("unterminated call")


LITERAL = re.compile(r'^"([^"\\]*)"$')
TERNARY = re.compile(r'^\w+\s*\?\s*"([^"\\]*)"\s*:\s*"([^"\\]*)"$')
IDENTIFIER = re.compile(r"^\w+$")


def resolve_identifier(name, source):
    """(literals, unresolved): what a local name can hold: `let n = "x"`, `let n = c ? "x" : "y"`, or the label of a
    `for (n, _) in [...]`; unresolved counts `let n = ...` definitions that are none of these. Every file is one
    namespace here, so a same-named `let` elsewhere that cannot be read must not hide behind one that can."""
    found, unresolved = [], 0
    for match in re.finditer(r'\blet %s\s*=\s*([^\n]+)' % re.escape(name), source):
        value = match.group(1).strip()
        literal, ternary = LITERAL.match(value), TERNARY.match(value)
        if literal:
            found.append(literal.group(1))
        elif ternary:
            found.extend(ternary.groups())
        else:
            unresolved += 1
    for match in re.finditer(r"\bfor \(%s, \w+\) in \[(.*?)\]\s*\{" % re.escape(name), source, re.S):
        found.extend(re.findall(r'\(\s*"([^"\\]*)"\s*,', match.group(1)))
    return found, unresolved


def collect(source):
    """(labels, problems): every label string SmokeQA looks for on screen, with how many call sites named it."""
    labels, problems = {}, []

    def add(label, where):
        labels.setdefault(label, []).append(where)

    for call in LABEL_CALLS:
        for match in re.finditer(r"(?<![\w.])(?<!func )%s\(" % call, source):
            line = source.count("\n", 0, match.start()) + 1
            argument = first_argument(source, match.end())
            where = "%s() line %d" % (call, line)
            literal, ternary = LITERAL.match(argument), TERNARY.match(argument)
            if literal:
                add(literal.group(1), where)
            elif ternary:
                for label in ternary.groups():
                    add(label, where)
            elif IDENTIFIER.match(argument) and resolve_identifier(argument, source)[0] and not resolve_identifier(argument, source)[1]:
                for label in resolve_identifier(argument, source)[0]:
                    add(label, where)
            else:
                problems.append("%s: cannot resolve the label argument %r to string literals; teach smokeqa-labels-check.py "
                                "how to read it" % (where, argument))
    for match in re.finditer(r"\bp15Host\(\s*\"([^\"]+)\"\s*,\s*stale:\s*\"([^\"]+)\"", source):
        for host in match.groups():
            add(host, "p15Host() line %d" % (source.count("\n", 0, match.start()) + 1))
    for match in re.finditer(r"\.contains\(\s*\"([^\"]+)\"", source):
        label = match.group(1)
        before = re.search(r"(\w+)$", source[: match.start()])
        receiver = before.group(1) if before else source[max(0, match.start() - 12) : match.start()].strip()
        line = source.count("\n", 0, match.start()) + 1
        if receiver in OCR_RECEIVERS:
            add(label, "%s.contains() line %d" % (receiver, line))
        elif receiver not in NOT_OCR_RECEIVERS:
            problems.append("%s.contains(%r) line %d: not known to be OCR output or not; decide in OCR_RECEIVERS / NOT_OCR_RECEIVERS"
                            % (receiver, label, line))
    return labels, problems


def locate(root, location):
    """(path, line) of the fragment, or None when the file or the fragment is missing."""
    path, fragment = location
    file = Path(root) / path
    if not file.is_file():
        return None
    text = file.read_text(errors="replace")
    if file.suffix == ".rs":
        text = strip_rust_comments(text)
    at = text.find(fragment)
    return None if at < 0 else (path, text.count("\n", 0, at) + 1)


def check(root, tests):
    """(rows, problems). rows are (label, kind, text) for the report."""
    source = strip_comments("\n".join(path.read_text() for path in sorted(Path(tests).glob("SmokeQA*.swift"))))
    labels, problems = collect(source)
    rows = []
    for label in sorted(labels):
        in_sources, in_allowed = label in SOURCES, label in ALLOWED
        if in_sources and in_allowed:
            problems.append("%r is in both SOURCES and ALLOWED" % label)
            continue
        if not in_sources and not in_allowed:
            problems.append("SmokeQA looks for %r (%s) but no app source or allow-list entry names it: the app may not show it. "
                            "Add the source location to SOURCES, or to ALLOWED with the reason" % (label, "; ".join(labels[label])))
            continue
        locations = SOURCES[label] if in_sources else ALLOWED[label][1]
        found = []
        for location in locations:
            hit = locate(root, location)
            if hit is None:
                problems.append("%r: %s no longer contains %r%s" % (
                    label, location[0], location[1], "" if in_sources else " (allow-list reason: %s)" % ALLOWED[label][0]))
            else:
                found.append("%s:%d" % hit)
        rows.append((label, "source" if in_sources else "allowed", ", ".join(found)
                     + ("" if in_sources else "  # " + ALLOWED[label][0])))
    for table, name in ((SOURCES, "SOURCES"), (ALLOWED, "ALLOWED")):
        for label in sorted(table):
            if label not in labels:
                problems.append("%s entry %r is no longer used by SmokeQA: delete it" % (name, label))
    return rows, problems, sum(len(v) for v in labels.values())


def main(argv=None):
    here = Path(__file__).resolve()
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", default=str(here.parents[2]))
    parser.add_argument("--tests", default=str(here.parent / "smoke-qa" / "Tests"))
    args = parser.parse_args(argv)
    rows, problems, call_sites = check(args.root, args.tests)
    for label, kind, text in rows:
        print("%-8s %-34r %s" % (kind, label, text))
    print("%d distinct strings from %d call sites: %d tied to a source, %d allowed"
          % (len(rows), call_sites, sum(1 for r in rows if r[1] == "source"), sum(1 for r in rows if r[1] == "allowed")))
    for problem in problems:
        print("ERROR", problem, file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
