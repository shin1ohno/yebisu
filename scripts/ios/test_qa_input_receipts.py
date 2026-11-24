"""Static guards for yebisu's intake of herdr-uikit's input receipts (issue #142 stage 2).

The receipt module and every call into it must stay out of a device build, and the global
`menu-global` listener must be registered once per process. Rust's cfg attributes are read as
text, so this runs on any host without a Simulator."""
from pathlib import Path
import re
import unittest

SRC = Path(__file__).resolve().parents[2] / "crates/yebisu/src"
SIMULATOR = '#[cfg(all(target_os = "ios", target_abi = "sim"))]'
SIMULATOR_OR_TEST = '#[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]'
CALLERS = ("diagnostics.rs", "app.rs", "terminal_view.rs")


def code_lines(text):
    """The lines of text without comment lines, keeping their order."""
    return [line.strip() for line in text.splitlines() if line.strip() and not line.strip().startswith("//")]


def nearest_cfg(lines, at, reach=2):
    """The first `#[cfg` line among the `reach` code lines above lines[at], or None."""
    for line in reversed(lines[max(0, at - reach):at]):
        if line.startswith("#[cfg"):
            return line
    return None


def body(text, signature):
    """The brace-balanced body of the first function whose declaration starts with signature."""
    start = text.index(signature)
    open_at = text.index("{", start)
    depth = 0
    for at in range(open_at, len(text)):
        depth += {"{": 1, "}": -1}.get(text[at], 0)
        if depth == 0:
            return text[open_at:at + 1]
    raise AssertionError("unbalanced function body: " + signature)


class ReceiptGates(unittest.TestCase):
    def test_the_module_is_built_only_for_the_simulator_and_tests(self):
        lines = code_lines((SRC / "qa_ax.rs").read_text())
        at = lines.index("pub(crate) mod input;")
        self.assertEqual(lines[at - 1], '#[path = "qa_ax_input.rs"]')
        self.assertEqual(lines[at - 2], SIMULATOR_OR_TEST)

    def test_every_call_into_the_module_is_simulator_only(self):
        found = 0
        for name in CALLERS:
            lines = code_lines((SRC / name).read_text())
            for at, line in enumerate(lines):
                if "qa_ax::input::" in line and not line.startswith("assert"):
                    found += 1
                    with self.subTest(file=name, line=line):
                        self.assertEqual(nearest_cfg(lines, at), SIMULATOR)
        # diagnostics: receipts_wanted and capture; app: heartbeat and register_global; terminal_view: drain.
        self.assertEqual(found, 5)

    def test_no_other_production_file_calls_into_the_module(self):
        callers = sorted(path.name for path in SRC.rglob("*.rs") if "qa_ax::input::" in path.read_text()
                         and not path.name.endswith("tests.rs") and path.name not in CALLERS)
        self.assertEqual(callers, [])

    def test_the_receipt_target_is_accepted_only_behind_the_launch_flag(self):
        text = (SRC / "diagnostics.rs").read_text()
        lines = code_lines(text)
        constant = lines.index('const RECEIPTS: &str = "herdr_uikit::qa_input";')
        self.assertEqual(lines[constant - 1], SIMULATOR_OR_TEST, "a device build must not carry the receipt target")
        self.assertIn("target == TARGET || (receipts && is_receipt(target))", text)
        device = '#[cfg(not(any(all(target_os = "ios", target_abi = "sim"), test)))]\nfn is_receipt(_: &str) -> bool {\n    false\n}'
        self.assertIn(device, text)
        self.assertIn("if is_receipt(event.metadata().target()) {", body(text, "fn event("))
        self.assertIn("accepts(metadata.target(), self.receipts)", body(text, "fn enabled("))
        self.assertIn("receipts: receipts_wanted(),", body(text, "pub(crate) fn init("))
        device = text[text.index('#[cfg(all(target_os = "ios", not(target_abi = "sim")))]'):]
        self.assertIn("false", body(device, "fn receipts_wanted("))

    def test_the_global_listener_is_registered_once_in_run_not_in_render(self):
        text = (SRC / "app.rs").read_text()
        run = body(text, "pub(crate) fn run(")
        self.assertEqual(run.count("qa_ax::input::register_global(cx)"), 1)
        self.assertLess(run.index("register_global(cx)"), run.index("cx.open_window("))
        self.assertEqual(text.count("register_global("), 1)
        self.assertNotIn("register_global", body(text, "fn render("))

    def test_the_drain_runs_before_the_refresh_of_each_poll(self):
        poll = body((SRC / "terminal_view.rs").read_text(), "pub(crate) fn poll_terminal(")
        self.assertLess(poll.index("qa_ax::input::drain(&self.observation)"), poll.index("qa_ax::refresh_if_wanted(cx)"))

    def test_the_heartbeat_runs_on_the_one_second_tick(self):
        text = (SRC / "app.rs").read_text()
        tick = text[text.index("Duration::from_secs(1)"):text.index("Duration::from_millis(33)")]
        self.assertIn("qa_ax::input::heartbeat(&this.observation, this.ticks)", tick)

    def test_the_module_keeps_no_string_it_was_sent(self):
        text = (SRC / "qa_ax_input.rs").read_text()
        receipt = text[text.index("pub(crate) enum Receipt"):text.index("/// The fields of one event")]
        self.assertNotIn("String", receipt)
        self.assertNotRegex(text, r"\.to_owned\(\)|\.to_string\(\)|String::from")
        self.assertIn("try_send(", text)
        self.assertNotRegex(text, r"\.send\(")


if __name__ == "__main__":
    unittest.main()
