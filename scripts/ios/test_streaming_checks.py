"""The streaming checks run their `xcodebuild test` with the keywords their call sites pass.

smoke-check, p9, p2, p3 and p10 stream `xcodebuild test` through resend_budget.run_tee (issue #142
stage 4), which hands its keywords to subprocess.Popen. p3-check.py passed `capture_output=True`, which
subprocess.run takes and Popen refuses, and layout-smoke never runs on a pull request (ci.yml), so P3
failed first on main (push run 45558087383, job 139015022407). Each test reads the keywords of the
script's `xcodebuild test` call from its source and calls the script's own run() helper with them;
Popen is the real one, running a stand-in for xcodebuild.
"""
import ast
import contextlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import sys
import unittest
from unittest import mock

HERE = Path(__file__).parent
STREAMING = ("smoke-check.py", "p9-check.py", "p2-check.py", "p3-check.py", "p10-check.py")
UDID = "99ECD66A-0000-4000-8000-000000000002"
PASSED = "Test Case '-[SmokeOracle.SmokeQA testP3Terminal]' passed (1.000 seconds)."
# A value of the type each keyword takes, for the keywords a call site may pass.
VALUES = {"env": lambda: dict(os.environ), "cwd": lambda: str(HERE), "text": lambda: True,
          "check": lambda: True, "capture_output": lambda: True, "timeout": lambda: 60}


def load(name):
    spec = importlib.util.spec_from_file_location(name[:-3].replace("-", "_"), HERE / name)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def starts_test(node):
    """Whether `node` is a list literal that starts "xcodebuild", "test" (or a tuple whose first item is)."""
    if isinstance(node, ast.Tuple) and node.elts:
        node = node.elts[0]
    words = getattr(node, "elts", [])[:2] if isinstance(node, ast.List) else []
    return [getattr(word, "value", None) for word in words] == ["xcodebuild", "test"]


def xcodebuild_test_calls(source):
    """The keyword names of each `run(...)` call in `source` that runs `xcodebuild test`: literal
    arguments, `*builder(...)` or `*name` where `name` was assigned from a builder whose return value
    starts with "xcodebuild", "test". A `**mapping` keyword is named None."""
    tree = ast.parse(source)
    builders = {node.name for node in ast.walk(tree) if isinstance(node, ast.FunctionDef)
                and any(isinstance(ret, ast.Return) and ret.value is not None and starts_test(ret.value)
                        for ret in ast.walk(node))}
    built = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Assign) and isinstance(node.value, ast.Call) and getattr(node.value.func, "id", None) in builders:
            for target in node.targets:
                first = target.elts[0] if isinstance(target, ast.Tuple) else target
                if isinstance(first, ast.Name):
                    built.add(first.id)
    calls = []
    for node in ast.walk(tree):
        if not (isinstance(node, ast.Call) and getattr(node.func, "id", None) == "run" and node.args):
            continue
        first = node.args[0]
        literal = [getattr(arg, "value", None) for arg in node.args[:2]] == ["xcodebuild", "test"]
        spread = isinstance(first, ast.Starred) and (
            (isinstance(first.value, ast.Call) and getattr(first.value.func, "id", None) in builders)
            or (isinstance(first.value, ast.Name) and first.value.id in built))
        if literal or spread:
            calls.append([keyword.arg for keyword in node.keywords])
    return calls


class CallSiteReaderTests(unittest.TestCase):
    """The reader finds the call shapes the checks use, so an empty result is not a silent pass."""

    def test_literal_builder_and_unpacked_builder_calls_are_found(self):
        source = '''
def build(a):
    return ["xcodebuild", "test", a], {"K": "v"}
def plain(a):
    return ["xcodebuild", "test", a]
def main():
    run("xcodebuild", "test", "-scheme", "S", capture_output=True)
    run(*plain(1))
    command, extra = build(2)
    run(*command, env=extra)
    run("xcodebuild", "build", check=True)
    run("bash", "x.sh", capture_output=True)
'''
        self.assertEqual(xcodebuild_test_calls(source), [["capture_output"], [], ["env"]])


class StreamingRunTests(unittest.TestCase):
    def test_these_are_all_the_streaming_checks(self):
        found = sorted(path.name for path in HERE.glob("*.py")
                       if not path.name.startswith("test_") and "resend_budget.run_tee(" in path.read_text())
        self.assertEqual(found, sorted(STREAMING))

    def test_each_xcodebuild_test_call_streams_with_its_own_keywords(self):
        real_popen = subprocess.Popen
        for name in STREAMING:
            with self.subTest(script=name):
                calls = xcodebuild_test_calls((HERE / name).read_text())
                self.assertEqual(len(calls), 1, "one xcodebuild test call")
                keywords = calls[0]
                self.assertTrue(all(keyword in VALUES for keyword in keywords),
                                f"give each keyword a value in VALUES: {keywords}")
                module = load(name)
                started, recorded = [], []

                def stand_in(command, **options):
                    started.append(list(command))
                    return real_popen([sys.executable, "-c", f"print({PASSED!r})"], **options)

                module.resend_budget.record = lambda check, output, ledger=None: recorded.append((check, output)) or []
                with mock.patch.object(subprocess, "Popen", stand_in), contextlib.redirect_stdout(io.StringIO()) as shown:
                    result = module.run("xcodebuild", "test", "-destination", "id=" + UDID,
                                        **{keyword: VALUES[keyword]() for keyword in keywords})
                self.assertEqual(result.returncode, 0)
                self.assertIn(PASSED, result.stdout)
                self.assertIn(PASSED, shown.getvalue(), "the output is streamed as it comes")
                self.assertEqual(started, [["xcodebuild", "test", "-destination", "id=" + UDID]])
                self.assertEqual(recorded, [(name[:-3], result.stdout)], "the check's resends reach the ledger")


if __name__ == "__main__":
    unittest.main()
