#!/usr/bin/env python3
"""#139: run testInputModeIndicator and judge T-IM-S1..S8 from what it recorded.

Builds the Simulator app, boots HERDR_IOS_SIMULATOR, gives it the input sources en_US and
ja_JP-Romaji (the global AppleKeyboards preference), installs the app, saves Shortcut style =
Herdr prefix (T-IM-S7), runs SmokeQA/testInputModeIndicator, then reads the INPUT_MODE_OBS lines
the test printed (qa.state.input_mode at each step) and its screenshots. Prints one
`INPUT_MODE <case> PASS|FAIL <reason>` line per case and writes summary.json with the
screenshots into --out. A Simulator result does not prove real IME or hardware keyboard
behaviour (AGENTS.md); the owner's iPad checks (D1-D7) do.

An app that never reports idle to XCUITest makes each action wait minutes (#212: the G14
mutation ran 5.5 h), so the test has an overall limit (--timeout, seconds). Past it, or once the
test printed the step --stop-after names, xcodebuild gets SIGTERM and the cases are judged from
what it printed by then; summary.json's `stopped` says which, and the run exits 1.

  input-mode-check.py [--out DIR] [--label L]      # just ios-input-mode; DIR: target/ios-input-mode
  input-mode-check.py --stop-after s2-h2-after     # a mutation leg judged by T-IM-S2 alone
"""
import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import threading
import time

KEYBOARDS = ['en_US@sw=QWERTY;hw=Automatic', 'ja_JP-Romaji@sw=QWERTY-Japanese;hw=Automatic']
CASES = ('T-IM-S1', 'T-IM-S2', 'T-IM-S3', 'T-IM-S4', 'T-IM-S5', 'T-IM-S6', 'T-IM-S7', 'T-IM-S8')
# The rows a sheet or panel switches the interaction off for (AC5): Settings, its Shortcuts
# and Hosts & keys pages, and the command palette (the ... menu's "Command palette" row).
# Find is not run here (AC5 does not list it; owner check D6 covers it).
OFF_WHILE_OPEN = ('s5-settings', 's5-shortcuts', 's5-hosts', 's5-palette')
# The whole test's limit. An unmutated run took 69 s at a load average near 130 (#153 run 11);
# the G14 mutation's first action alone waited 120 s.
TIMEOUT = 1800
# How long xcodebuild gets to end after SIGTERM before SIGKILL; it ended in about 10 s (#153).
GRACE = 60


def observations(stdout):
    """{step: record} from the test's INPUT_MODE_OBS lines; a repeated step keeps the last."""
    found = {}
    for line in stdout.splitlines():
        marker = line.find('INPUT_MODE_OBS ')
        if marker < 0:
            continue
        try:
            record = json.loads(line[marker + len('INPUT_MODE_OBS '):])
        except ValueError:
            continue
        if isinstance(record, dict) and isinstance(record.get('step'), str):
            found[record['step']] = record
    return found


def mode(obs, step):
    record = obs.get(step) or {}
    value = record.get('input_mode')
    return value if isinstance(value, dict) else {}


def active(obs, step):
    m = mode(obs, step)
    return m.get('text_first_responder') is True and m.get('activated') is True


def inactive(obs, step):
    m = mode(obs, step)
    return bool(m) and m.get('activated') is False


def rect(value):
    if isinstance(value, list) and len(value) == 4 and all(isinstance(v, (int, float)) for v in value):
        return [float(v) for v in value]
    return None


def adjacent(accessory, caret):
    """The indicator touches the caret: its top within [bottom - 1, bottom + 8] of the caret and
    horizontally overlapping [caret.x - 40, caret.right + 160] (AC4; the SDK fixture's rule)."""
    accessory, caret = rect(accessory), rect(caret)
    if accessory is None or caret is None:
        return False
    bottom = caret[1] + caret[3]
    left, right = caret[0] - 40, caret[0] + caret[2] + 160
    return bottom - 1 <= accessory[1] <= bottom + 8 and accessory[0] < right and accessory[0] + accessory[2] > left


def follows(obs, step, tolerance=0.5):
    """caret_rect's origin within `tolerance` points of composition_bounds' origin (AC8)."""
    m = mode(obs, step)
    caret, bounds = rect(m.get('caret_rect')), rect(m.get('composition_bounds'))
    if caret is None or bounds is None:
        return False
    return abs(caret[0] - bounds[0]) <= tolerance and abs(caret[1] - bounds[1]) <= tolerance


def indicator(obs, step):
    """(frame, caret) of the indicator a switch showed, or None: on screen at the read, or else
    counted by the app's 33 ms poll since `before-<step>` (`shown`); XCUITest can wait 60 s for
    the app to idle after the key, past the indicator's display."""
    m = mode(obs, step)
    if rect(m.get('accessory_frame')):
        return m['accessory_frame'], m.get('caret_rect')
    now, before = m.get('shown'), mode(obs, f'before-{step}').get('shown')
    if not (isinstance(now, dict) and isinstance(before, dict)):
        return None
    if not (isinstance(now.get('count'), int) and isinstance(before.get('count'), int)):
        return None
    if now['count'] <= before['count'] or not rect(now.get('frame')):
        return None
    return now['frame'], now.get('caret')


def shown_switches(obs, prefix):
    steps = sorted(s for s in obs if s.startswith(prefix))
    return steps, [s for s in steps if indicator(obs, s)]


def judge_s1(obs):
    if not active(obs, 's1-focused'):
        return False, 'precondition: text view not first responder or interaction off'
    steps, shown = shown_switches(obs, 's1-switch-')
    if len(steps) != 3:
        return False, f'{len(steps)} of 3 switches recorded'
    far = [s for s in shown if not adjacent(*indicator(obs, s))]
    if len(shown) < 2 or far:
        return False, f'{len(shown)}/3 switches showed the indicator; not at the caret: {far}'
    return True, f'{len(shown)}/3 switches showed the indicator at the caret'


def judge_s2(obs):
    reasons = []
    for surface, before, after in (('Herdr 1', 's2-h1-before', 's2-h1-after'),
                                   ('Herdr 2', 's2-h2-before', 's2-h2-after')):
        if not active(obs, after):
            reasons.append(f'{surface}: precondition failed after the move')
            continue
        moved = rect(mode(obs, before).get('caret_rect')) != rect(mode(obs, after).get('caret_rect'))
        if not moved:
            reasons.append(f'{surface}: caret_rect did not move')
        elif not follows(obs, after):
            reasons.append(f'{surface}: caret_rect is not within 0.5 pt of composition_bounds')
    return not reasons, '; '.join(reasons) or 'caret_rect followed the cursor on Herdr 1 and Herdr 2'


def judge_s3(obs):
    ok = active(obs, 's3-foreground')
    return ok, 'active again after background and foreground' if ok else 'not active after foreground'


def judge_s4(obs):
    composing = mode(obs, 's4-composing').get('marked')
    after = mode(obs, 's4-after-output').get('marked')
    if not isinstance(composing, str) or not composing:
        return False, 'no composition: the Japanese input source did not compose (not observable)'
    if rect(mode(obs, 's4-composing').get('cursor')) == rect(mode(obs, 's4-after-output').get('cursor')):
        return False, 'no output moved the cursor while composing'
    if after != composing:
        return False, f'marked text {composing!r} became {after!r} after output'
    return True, f'marked text {composing!r} survived output that moved the cursor'


def judge_s5(obs):
    reasons = []
    for row in OFF_WHILE_OPEN:
        if not inactive(obs, f'{row}-open'):
            reasons.append(f'{row}: on while open')
        if not active(obs, f'{row}-closed'):
            reasons.append(f'{row}: off after close')
    return not reasons, '; '.join(reasons) or 'off while Settings, Shortcuts, Hosts & keys, palette open; on after'


def judge_s6(obs):
    if not active(obs, 's6-keyboard'):
        return False, 'precondition: not active with the software keyboard up'
    if not (active(obs, 's6-latched') and active(obs, 's6-after-letter')):
        return False, 'the ctrl latch or the letter changed the interaction'
    if not inactive(obs, 's6-hidden'):
        return False, 'Hide did not switch the interaction off'
    return True, 'latch and letter left it on; Hide switched it off'


def judge_s7(obs):
    if not active(obs, 's7-prefix-armed'):
        return False, 'precondition: not active with the prefix armed'
    if not active(obs, 's7-switch'):
        return False, 'the interaction went off after the switch'
    if not indicator(obs, 's7-switch'):
        return False, 'no indicator for the switch with the prefix armed'
    return True, 'the indicator followed the input source with the prefix armed'


def tinted_run(pixels, scale, caret, pad=4.0):
    """The widest run of adjacent columns, in points, within `caret` padded by `pad` points where
    at least 40% of the rows are the system tint (the SDK fixture's rule). `pixels(x, y)` gives
    (r, g, b)."""
    caret = rect(caret)
    if caret is None:
        return None
    x0, x1 = int(round((caret[0] - pad) * scale)), int(round((caret[0] + caret[2] + pad) * scale))
    y0, y1 = int(round((caret[1] - pad) * scale)), int(round((caret[1] + caret[3] + pad) * scale))
    rows = max(1, y1 - y0)
    best = run = 0
    for x in range(x0, x1):
        tinted = 0
        for y in range(y0, y1):
            r, g, b = pixels(x, y)
            tinted += b >= 150 and b > r + 60 and b > g + 20
        run = run + 1 if tinted * 10 >= rows * 4 else 0
        best = max(best, run)
    return best / scale


def judge_s8(obs, screenshots):
    """screenshots: [(width, height, pixels)] in pixels; scale from the window's width in points."""
    if not active(obs, 's8-cursor'):
        return False, 'precondition: not active'
    caret = mode(obs, 's8-cursor').get('caret_rect')
    window = (obs.get('s8-cursor') or {}).get('window')
    if not screenshots or not isinstance(window, list) or not window or not window[0]:
        return False, 'no screenshots or window size'
    runs = []
    for width, height, pixels in screenshots:
        scale = width / float(window[0])
        runs.append(tinted_run(pixels, scale, caret))
    if any(run is None for run in runs):
        return False, 'no caret_rect'
    widest = max(runs)
    return widest < 1.0, f'widest tinted run {widest:.1f} pt over {len(runs)} screenshots at {caret}'


def load_screenshots(paths):
    spec = importlib.util.spec_from_file_location('screenshot_color', Path(__file__).with_name('screenshot-color.py'))
    color = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(color)
    shots = []
    for path in paths:
        width, height, channels, rows = color.decode(path.read_bytes())

        def pixels(x, y, rows=rows, channels=channels, width=width, height=height):
            if not (0 <= x < width and 0 <= y < height):
                return (0, 0, 0)
            row = rows[y]
            return tuple(row[x * channels: x * channels + 3])
        shots.append((width, height, pixels))
    return shots


def judge(obs, screenshots):
    return {
        'T-IM-S1': judge_s1(obs), 'T-IM-S2': judge_s2(obs), 'T-IM-S3': judge_s3(obs),
        'T-IM-S4': judge_s4(obs), 'T-IM-S5': judge_s5(obs), 'T-IM-S6': judge_s6(obs),
        'T-IM-S7': judge_s7(obs), 'T-IM-S8': judge_s8(obs, screenshots),
    }


def printed_step(line, step):
    """Whether `line` is the INPUT_MODE_OBS line of `step`."""
    return step in observations(line)


def run_test(command, cwd, timeout, stop_after=None, grace=GRACE):
    """Runs `command` as subprocess.run(capture_output=True) would, but sends SIGTERM once
    `timeout` seconds pass or stdout has the INPUT_MODE_OBS line of `stop_after`, and SIGKILL
    `grace` seconds later. Returns (returncode, stdout, stderr, stopped): `stopped` is None for
    a test that ended by itself, else {'reason': 'timeout'|'step', 'step', 'after_s'}."""
    started = time.monotonic()
    process = subprocess.Popen(command, cwd=cwd, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    out, err, reached = [], [], threading.Event()

    def read_out():
        for line in process.stdout:
            out.append(line)
            if stop_after and printed_step(line, stop_after):
                reached.set()

    def read_err():
        err.extend(process.stderr)

    readers = [threading.Thread(target=read_out, daemon=True), threading.Thread(target=read_err, daemon=True)]
    for reader in readers:
        reader.start()
    stopped = None
    while process.poll() is None:
        if reached.wait(0.2):
            stopped = {'reason': 'step', 'step': stop_after}
        elif time.monotonic() - started >= timeout:
            stopped = {'reason': 'timeout', 'step': None}
        if stopped is None:
            continue
        stopped['after_s'] = round(time.monotonic() - started, 1)
        process.send_signal(signal.SIGTERM)
        try:
            process.wait(grace)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        break
    # A child xcodebuild left behind can hold the pipes open: keep what was read by then.
    for reader, stream in zip(readers, (process.stdout, process.stderr)):
        reader.join(grace if stopped else None)
        if not reader.is_alive():
            stream.close()
    return process.returncode, ''.join(out), ''.join(err), stopped


def run(*command, **kwargs):
    print('INPUT_MODE COMMAND', list(command), flush=True)
    return subprocess.run(command, check=True, text=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, default=Path('target/ios-input-mode'))
    parser.add_argument('--label', default='local')
    parser.add_argument('--timeout', type=float, default=TIMEOUT,
                        help=f'seconds the test may run before SIGTERM (default {TIMEOUT})')
    parser.add_argument('--stop-after', metavar='STEP',
                        help='SIGTERM the test once it printed this INPUT_MODE_OBS step, e.g. s2-h2-after')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    out = (args.out if args.out.is_absolute() else root / args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    app = run('bash', 'scripts/ios/bundle.sh', '--package', 'yebisu', '--bin', 'yebisu', capture_output=True, cwd=root)
    print(app.stderr[-4000:], flush=True)
    app_path = app.stdout.strip().splitlines()[-1]
    resolved = subprocess.check_output(['bash', '-c', 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"'],
                                       text=True, cwd=root)
    device = resolved.split()[0]
    run('xcrun', 'simctl', 'spawn', device, 'defaults', 'write', '-g', 'AppleKeyboards', '-array', *KEYBOARDS)
    run('xcrun', 'simctl', 'spawn', device, 'defaults', 'write', '-g', 'AppleKeyboardsExpanded', '-int', '1')
    subprocess.run(['xcrun', 'simctl', 'terminate', device, 'com.shin1ohno.yebisu'], text=True, capture_output=True)
    run('xcrun', 'simctl', 'install', device, app_path)
    container = Path(subprocess.check_output(['xcrun', 'simctl', 'get_app_container', device, 'com.shin1ohno.yebisu',
                                              'data'], text=True).strip())
    settings = container / 'Library/Application Support/herdr/gpui/yebisu-settings.json'
    settings.parent.mkdir(parents=True, exist_ok=True)
    # The whole document: a partial one is refused and the defaults apply (prefix-check.py).
    settings.write_text(json.dumps({'version': 1, 'language': 'en-US', 'compact': False, 'terminal_priority': False,
                                    'board_hidden': False, 'drag_select': True,
                                    'shortcut_style': 'herdr-prefix'}) + '\n')
    spec = root / 'scripts/ios/smoke-qa'
    run('xcodegen', 'generate', '--spec', str(spec / 'project.yml'), '--project', str(spec))
    result = out / 'result.xcresult'
    if result.exists():
        shutil.rmtree(result)
    returncode, stdout, stderr, stopped = run_test(
        ['xcodebuild', 'test', '-project', str(spec / 'SmokeOracle.xcodeproj'), '-scheme', 'SmokeOracle',
         '-destination', 'id=' + device, '-derivedDataPath', str(root / 'target/ios-smoke/build'),
         '-resultBundlePath', str(result), 'CODE_SIGNING_ALLOWED=NO',
         '-only-testing:SmokeOracle/SmokeQA/testInputModeIndicator'],
        root, args.timeout, args.stop_after)
    (out / 'xctest.stdout').write_text(stdout)
    (out / 'xctest.stderr').write_text(stderr)
    if stopped:
        print('INPUT_MODE STOPPED', stopped['reason'], stopped['step'] or f'{args.timeout:g} s',
              f"after {stopped['after_s']} s", flush=True)
    subprocess.run(['xcrun', 'simctl', 'terminate', device, 'com.shin1ohno.yebisu'], text=True, capture_output=True)
    export = out / 'attachments'
    if export.exists():
        shutil.rmtree(export)
    subprocess.run(['xcrun', 'xcresulttool', 'export', 'attachments', '--path', str(result), '--output-path',
                    str(export)], text=True, capture_output=True)
    shots = []
    manifest = export / 'manifest.json'
    if manifest.is_file():
        for case in json.loads(manifest.read_text()):
            for attachment in case['attachments']:
                name = attachment['suggestedHumanReadableName'].split('_0_')[0]
                target = out / (name + '.png')
                shutil.copyfile(export / attachment['exportedFileName'], target)
                if name.startswith('input-mode-s8-'):
                    shots.append(target)
    obs = observations(stdout)
    verdicts = judge(obs, load_screenshots(sorted(shots)))
    for case in CASES:
        ok, reason = verdicts[case]
        print('INPUT_MODE', case, 'PASS' if ok else 'FAIL', reason, flush=True)
    summary = {'label': args.label, 'xcodebuild_exit': returncode, 'stopped': stopped, 'observations': obs,
               'cases': {case: {'pass': ok, 'reason': reason} for case, (ok, reason) in verdicts.items()},
               'scope': 'Simulator only; not proof of real IME or hardware keyboard behaviour'}
    (out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    sys.exit(0 if returncode == 0 and not stopped and all(ok for ok, _ in verdicts.values()) else 1)


if __name__ == '__main__':
    main()
