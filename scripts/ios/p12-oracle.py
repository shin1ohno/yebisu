"""Joint installed-GUI, consumer and private-peer oracle. No model-only UI pass."""
import re

CASES = ["remote", "sidebar", "rearm", "palette", "host", "pane", "boot", "focus", "capability"]
NEGATIVE = CASES[4:]
LABELS = {"remote": "Split Right", "rearm": "Split Right", "sidebar": "Toggle Sidebar", "palette": "Command Palette"}
# Each remote pick's split is held for exactly one local case, then released.
HOLDS = [("remote", "sidebar"), ("rearm", "palette")]
# sheprd-client limits.rs COMMAND_TIMEOUT: a held request this old fails the link.
COMMAND_TIMEOUT = 60
# The fixture control steps P12QA taps, in order. Each is one `p12-setup-begin`/`p12-setup-end` pair.
STEPS = [*[s for hold in HOLDS for s in (*hold, "release")],
         *[s for case in NEGATIVE for s in ((case, "current", "reset") if case == "focus" else (case, "reset"))], "finish"]
# One swipe reveal and one menu close per choose() call, in the order P12QA runs them.
CHOOSES = [*CASES[:CASES.index("focus") + 1], "focus-current", *CASES[CASES.index("focus") + 1:]]
# A menu build this close to a tap is reported next to a lost tap.
BUILD_MARGIN = 1.0


def require(condition, message):
    if not condition:
        raise ValueError(message)


def is_image(data):
    return data.startswith(b"\x89PNG\r\n\x1a\n") or data.startswith(b"\xff\xd8")


def attachment_named(name, expected):
    return name.startswith(expected + "_") or name.startswith(expected + ".")


def hierarchy_key(case):
    return f"p12-menu-{case}-hierarchy"


def hierarchy_evidence(name, data):
    """Evidence tokens of one Springboard AX hierarchy attachment, or none.

    While the OS menu is open the oracle takes no screenshot (they are failure
    evidence only); the menu's proof is the Springboard hierarchy saved right after
    the menu opened. A token is returned only when that text, in its SPRINGBOARD
    section, lists the case's native command as a button: `<key>` for the item being
    listed and `<key>-enabled` when it is not marked Disabled."""
    if is_image(data):
        return set()
    for case in [*CASES, "focus-current"]:
        key = hierarchy_key(case)
        if not attachment_named(name, key):
            continue
        sections = data.decode("utf-8", "replace").split("\nSPRINGBOARD\n", 1)
        if len(sections) != 2:
            return set()
        label = re.escape(LABELS.get(case, "Split Right"))
        # Same label rule as the os-menu-pick check: the command, then an optional shortcut suffix.
        items = re.findall(rf"Button, [^\n]*label: '{label}(?:[\u3000,][^']*)?'([^\n]*)", sections[1])
        if not items:
            return set()
        return {key, *([key + "-enabled"] if not any("Disabled" in flags for flags in items) else [])}
    return set()


def screenshot_names(name, data):
    """Evidence names one exported attachment proves: real screenshots and the menu hierarchies."""
    if not is_image(data):
        return hierarchy_evidence(name, data)
    # The three screenshots taken while the OS menu is open are failure evidence only.
    expected = ["p12-finished", "p12-menu-focus-current", *[f"p12-after-{case}" for case in CASES]]
    return {item for item in expected if attachment_named(name, item)}


def after_launch_retry(messages, ui):
    """The peer messages of the launch the test ran on. A no-pid launch-retry (issue #247) starts the
    app twice; the app writes a new app.jsonl at each launch, but the peer log keeps the first
    launch's start and geometry (dispatch 46098587323). So after a retry only the messages from the
    one p12-peer-start at or after the retry on are evidence; without a retry all of them are."""
    retries = [r for r in ui if r.get("kind") == "launch-retry"]
    if not retries:
        return messages
    at = retries[-1]["time"]
    starts = [index for index, m in enumerate(messages) if m.get("kind") == "p12-peer-start" and m.get("time", 0) >= at]
    require(len(starts) == 1, "peer reconnected or start missing after the launch retry")
    return messages[starts[0]:]


def validate(records, messages, ui, screenshots):
    messages = after_launch_retry(messages, ui)
    picks = [r for r in ui if r.get("kind") == "os-menu-pick"]
    require([r.get("case") for r in picks] == CASES, "actual OS menu cases missing/duplicated")
    validate_fixture_taps(records, ui)
    retries = validate_retries(ui)
    validate_menu_retries(records, ui)
    prepares = [r for r in records if r.get("kind") == "p12-prepare"]
    require([r.get("case") for r in prepares] == CASES, "app prepare cases missing/duplicated")
    receivers = [r for r in records if r.get("kind") == "p12-menu-receiver"]
    require(all(r.get("case") in CASES for r in receivers), "unexpected native callback")
    validate_gui_preconditions(records, ui, picks, prepares)
    validate_observation(records, messages, ui, picks, prepares, receivers)
    negative_routes = {}
    for case, pick, prepare in zip(CASES, picks, prepares):
        require(pick.get("owner") in ["com.apple.springboard", "com.shin1ohno.yebisu"], "wrong native owner")
        label = LABELS.get(case, "Split Right")
        require(pick.get("command") == label and pick.get("buttonLabel", "").split("　")[0].split(",")[0] == label, "wrong native command label")
        require(f"p12-after-{case}" in screenshots, "real screenshot missing")
        require(hierarchy_key(case) in screenshots, f"Springboard AX hierarchy item missing in case {case}")
        if case not in NEGATIVE:
            # A negative case's menu is captured before the stale fault lands, so only its listing is required.
            require(hierarchy_key(case) + "-enabled" in screenshots, f"native item disabled in the AX hierarchy in case {case}")
        matches = [r for r in prepare["catalog"] if r["label"] == label]
        require(len(matches) == 1 and matches[0]["enabled"], "prepared catalog mismatch")
        received = [r for r in receivers if r["case"] == case]
        if case not in NEGATIVE:
            require(pick.get("enabled") is True and pick.get("tapped") is True, "positive native item was not tapped")
            require(len(received) == 1 and received[0].get("accepted") is True, "positive dispatch not exactly once")
            result = received[0]
            require(result["name"] == matches[0]["name"] and result["pending"], "wrong action or pending gate lost")
            require(prepare["pending"] == (case not in dict(HOLDS)), "local case was not tested during pending RPC")
            before = [prepare["sidebar"], prepare["palette"]]
            expected = list(before)
            if case in ["sidebar", "palette"]:
                index = 0 if case == "sidebar" else 1
                expected[index] = not before[index]
            require(result["before"] == before and result["after"] == expected, "local state did not toggle exactly once")
        else:
            mutations = [r for r in records if r.get("kind") == "p12-mutated" and r.get("case") == case]
            resets = [r for r in records if r.get("kind") == "p12-reset" and r.get("case") == case]
            require(len(mutations) == 1 and len(resets) == 1 and resets[0]["mutated"], "stale fault not observed/restored")
            before = [prepare["sidebar"], prepare["palette"]]
            require([resets[0]["sidebar"], resets[0]["palette"]] == before, "stale command changed local state")
            if case == "focus":
                require(pick.get("tapped") is True, "valid focus stale menu must reach consumer rejection")
                require(mutations[0].get("valid_focus_target") is True and mutations[0].get("current_split_enabled") is True,
                        "focus must move to an available existing pane")
                current = [r for r in records if r.get("kind") == "p12-current"]
                shown = [r for r in ui if r.get("kind") == "current-native-menu"]
                control = next(c for c in ui if c.get("kind") == "fixture-control" and c["step"] == "current")
                require(len(current) == len(shown) == 1 and current[0]["case"] == shown[0]["case"] == "focus",
                        "current focus menu observation missing")
                require(inside(current[0], control) and pick["ended"] < control["started"] < control["ended"] < shown[0]["time"],
                        "current menu must follow stale rejection and rebuild")
                require(current[0]["state"] == mutations[0]["state"], "current menu changed valid focus")
                require(shown[0].get("enabled") is True and shown[0].get("owner") == "com.apple.springboard"
                        and shown[0].get("buttonLabel", "").split("　")[0] == "Split Right", "current native menu unavailable")
                require("p12-menu-focus-current" in screenshots, "current native menu screenshot missing")
                require(hierarchy_key("focus-current") + "-enabled" in screenshots, "current native menu AX hierarchy item missing")
            if pick.get("tapped"):
                require(pick.get("enabled") is True and len(received) == 1, "stale enabled GUI pick lacks one receiver")
                negative_routes[case] = "consumer_rejected_actual_os_pick"
                result = received[0]
                require(result.get("accepted") is False and result.get("mutated") is True, "stale consumer guard accepted")
                require(result["name"] == matches[0]["name"] and result["before"] == result["after"] == before, "wrong stale receiver/state")
            else:
                require(pick.get("enabled") is False and not received, "untapped item is not native disabled evidence")
                negative_routes[case] = "native_gui_disabled_no_consumer_dispatch"
    require([r.get("released") for r in records if r.get("kind") == "p12-release"] == [True] * len(HOLDS), "peer gate not explicitly released")
    require([r.get("enqueued") for r in records if r.get("kind") == "p12-finish"] == [True], "fixture fence not queued")
    require("p12-finished" in screenshots, "final screenshot missing")
    return {"installed_os_menu": "pass", "consumer_peer": "pass", "command_hold": "unverified",
            "physical_keyboard_ime": "unverified", "p11_p13_modal_integration": "unverified", "negative_routes": negative_routes,
            "retries": retries, "hold_queries": hold_queries(ui), "input_receipts": input_receipts(records)}


def input_receipts(records):
    """How many herdr-uikit input receipts of each kind the app recorded (issue #142 stage 2).
    Reported, not judged: it shows the receipts reached the app's file."""
    counts = {}
    for record in records:
        if record.get("kind") == "input-receipt":
            counts[record.get("receipt")] = counts.get(record.get("receipt"), 0) + 1
    return dict(sorted(counts.items(), key=lambda item: str(item[0])))


def menu_stage(records, pick):
    """Where a pick with no p12-menu-entry stopped, by the app's input receipts inside the pick window.

    The names, in the order they are decided: `menu-global` (the command reached GPUI, but no window
    handler on the dispatch path took it), `menu-deliver <outcome>` for the first deliver that did not
    deliver, `menu-perform <outcome>` for a perform without a tag or sender, `no menu-perform` (the
    command never reached the app: SpringBoard did not perform it, or the touch never reached the menu),
    `menu-perform without menu-deliver` (performed, never handed to GPUI), else `menu-deliver delivered`."""
    receipts = [r for r in records if r.get("kind") == "input-receipt" and inside(r, pick)]
    performs = [r for r in receipts if r.get("receipt") == "menu-perform"]
    delivers = [r for r in receipts if r.get("receipt") == "menu-deliver"]
    if any(r.get("receipt") == "menu-global" for r in receipts):
        return "menu-global"
    failed = [r for r in delivers if r.get("outcome") != "delivered"]
    if failed:
        return f"menu-deliver {failed[0].get('outcome')}"
    refused = [r for r in performs if r.get("outcome") != "performed"]
    if refused:
        return f"menu-perform {refused[0].get('outcome')}"
    if not performs:
        touches = [r for r in receipts if r.get("receipt") == "touch" and r.get("stage") == "began"]
        if touches:
            last = touches[-1]
            return (f"pick tap reached the app ({len(touches)} touch began, last at {last.get('x')}, {last.get('y')}), "
                    f"not the menu item")
        return "no menu-perform"
    if not delivers:
        return "menu-perform without menu-deliver"
    return "menu-deliver delivered"


# A hold window longer than this leaves under 20 s of the 60 s COMMAND_TIMEOUT: the S5 follow-up
# said to act once a hold passes 40 s, so the verdict says so by name.
HOLD_WARNING_SECONDS = 40


def hold_queries(ui):
    """The qa.state reads P12QA made while each request was held, with their seconds and the
    hold window's length (from the end of the Remote/Rearm pick, which sends the held pane.split,
    to Release's confirmation). UI test redesign S6, S5 follow-up S6-1: fewer queries inside a hold make
    an XCTest idle wait (60-80 s on an old runner image) less likely to land there, but one such
    wait still would push the request past the 60 s COMMAND_TIMEOUT, so the reads are measured,
    and a window above HOLD_WARNING_SECONDS is named in `warning`. Reported, not judged."""
    result = []
    for r in (r for r in ui if r.get("kind") == "hold-queries"):
        hold = round(r.get("ended", 0) - r.get("started", 0), 2)
        entry = {"case": r.get("case"), "reads": r.get("reads"), "read_seconds": round(r.get("readSeconds", 0), 2),
                 "hold_seconds": hold}
        if hold > HOLD_WARNING_SECONDS:
            entry["warning"] = f"hold window {hold} s is above {HOLD_WARNING_SECONDS} s of the 60 s COMMAND_TIMEOUT"
        result.append(entry)
    return result


def scope_text(record):
    """What a `rebuild` record says the menu was rebuilt from: a Scope-only rebuild differs here, not in rows or focus."""
    build = record.get("build")
    if not isinstance(build, dict):
        return ""
    return ", " + ", ".join(f"{key}={build.get(key)}" for key in ("host", "boot", "pane", "epoch"))


def builds_near(records, time):
    """The p12-menu-build records within BUILD_MARGIN of a tap, for the failure text.
    A `rebuild` record is written for every OS menu rebuild; prepare/current are setup-step records."""
    near = [r for r in records if r.get("kind") == "p12-menu-build" and isinstance(r.get("time"), (float, int))
            and isinstance(time, (float, int)) and abs(r["time"] - time) <= BUILD_MARGIN]
    return "; ".join(f"{r.get('source')} build at {r['time'] - time:+.2f} s (changed={r.get('changed')}"
                     f"{scope_text(r)})" for r in near) or \
        f"no p12-menu-build within {BUILD_MARGIN} s"


def validate_fixture_taps(records, ui):
    """Name the fixture tap the app never received (class P12-fixture).

    P12QA taps a fixture control and waits for the app's own `p12-setup-end`; a tap
    with no record is repeated once. In job 134059155142 the first tap (Remote) never
    reached the app and the first symptom was the generic 'app prepare cases
    missing/duplicated'. A control window with no `p12-setup-begin` of its step is
    that tap, so it is reported by step before the generic checks."""
    controls = [r for r in ui if r.get("kind") == "fixture-control"]
    begins = [r for r in records if r.get("kind") == "p12-setup-begin"]
    for control in controls:
        require(any(inside(begin, control) and begin.get("step") == control.get("step") for begin in begins),
                f"fixture tap lost (class P12-fixture): the app logged no p12-setup-begin for step {control.get('step')} "
                f"during its control window; steps with a begin: {sorted({b.get('step') for b in begins})}")
        require(control.get("received") is True,
                f"fixture tap on step {control.get('step')} has no app receipt in the XCTest record (class P12-fixture)")


def validate_retries(ui):
    """A fixture tap or a reveal swipe may be repeated once, with its reason logged.

    The retries are returned for the verdict, so a repeat is never silent and the
    rate of flaky taps and swipes stays visible. The assertion that a control was
    received and that the menu bar was revealed is not relaxed by a retry: the
    repeat has to succeed, and `validate_fixture_taps` and the reveal check still
    apply to its final state."""
    controls = [r for r in ui if r.get("kind") == "fixture-control"]
    taps = [r for r in ui if r.get("kind") == "fixture-retry"]
    for retry in taps:
        require(isinstance(retry.get("reason"), str) and retry["reason"], "fixture retry without a logged reason")
        require(retry.get("step") in STEPS and isinstance(retry.get("injected"), bool), "fixture retry record incomplete")
    for control in controls:
        require(len([r for r in taps if r.get("step") == control.get("step") and inside(r, control)]) <= 1,
                f"more than one fixture retry for step {control.get('step')}")
    require(all(any(inside(r, control) and r.get("step") == control.get("step") for control in controls) for r in taps),
            "fixture retry outside its control window")
    swipes = [r for r in ui if r.get("kind") == "reveal-retry"]
    for retry in swipes:
        require(isinstance(retry.get("reason"), str) and retry["reason"], "reveal retry without a logged reason")
        require(retry.get("case") in CHOOSES and isinstance(retry.get("injected"), bool), "reveal retry record incomplete")
    require(len({r.get("case") for r in swipes}) == len(swipes), "more than one reveal retry for a case")
    for result in (r for r in ui if r.get("kind") == "reveal-result"):
        retried = len([r for r in swipes if r.get("case") == result.get("case")])
        require(result.get("attempts", 1) == 1 + retried, f"reveal attempts and retries disagree in case {result.get('case')}")
    menus = {}
    for kind, key in (("pick-retry", "pick"), ("open-retry", "open")):
        found = [r for r in ui if r.get("kind") == kind]
        for retry in found:
            require(isinstance(retry.get("reason"), str) and retry["reason"], f"{kind} without a logged reason")
            require(retry.get("case") in CHOOSES and isinstance(retry.get("injected"), bool), f"{kind} record incomplete")
        require(len({r.get("case") for r in found}) == len(found), f"more than one {kind} for a case")
        menus[key] = [{"case": r["case"], "reason": r["reason"], "injected": r["injected"]} for r in found]
    # Issue #247: the one launch may be repeated once, only as no-pid-app-running (P12QA.launchOnce).
    launches = [r for r in ui if r.get("kind") == "launch-retry"]
    for retry in launches:
        require(isinstance(retry.get("reason"), str) and retry["reason"].startswith(NO_PID_APP_RUNNING + ":"),
                "launch retry without its no-pid-app-running reason")
        require(retry.get("case") == LAUNCH and isinstance(retry.get("injected"), bool), "launch retry record incomplete")
    require(len(launches) <= 1, "more than one launch retry")
    menus["launch"] = [{"case": r["case"], "reason": r["reason"], "injected": r["injected"]} for r in launches]
    return {"fixture": [{"step": r["step"], "reason": r["reason"], "injected": r["injected"]} for r in taps],
            "reveal": [{"case": r["case"], "reason": r["reason"], "injected": r["injected"]} for r in swipes], **menus}


def pick_decision(closed, entry, perform_grew, touch_grew, screen_moved, receipts_bad):
    """Issue #142 stage 3's decision table for a pick, as P12QA's MenuInputDecision.pick decides it
    (QAAx.swift) and the oracle judges a closed menu. Returns (verdict, resend):
    success, A1 (open 5 s and nothing reached the app: tap the same item once more), A2 (closed
    with no callback: not resent, the selection or its delivery was lost), defect (no_tag, a failed
    menu-deliver or menu-global: not resent), or not-lost (open, but the app received something)."""
    if receipts_bad:
        return "defect", False
    if closed:
        return ("success", False) if entry else ("A2", False)
    if perform_grew or touch_grew or screen_moved:
        return "not-lost", False
    return "A1", True


def open_decision(item_exists, touch_grew, screen_moved):
    """The decision table's row 5 for the tap that opens the OS menu (MenuInputDecision.open): no
    menu item, no app touch and no screen change is class M, and the open tap is sent once more."""
    if item_exists:
        return "opened", False
    if touch_grew or screen_moved:
        return "not-lost", False
    return "M", True


def validate_menu_retries(records, ui):
    """A pick-retry or open-retry is accepted only when the app's receipts agree it was lost: no
    touch began and no menu-perform between the first input and the retry (issue #142 stage 3)."""
    picks = {r.get("case"): r for r in ui if r.get("kind") == "os-menu-pick"}
    reveals = {r.get("case"): r for r in ui if r.get("kind") == "reveal-result"}
    receipts = [r for r in records if r.get("kind") == "input-receipt"]
    for retry in (r for r in ui if r.get("kind") in ("pick-retry", "open-retry")):
        case = retry.get("case")
        start = (picks.get(case) or {}).get("started") if retry["kind"] == "pick-retry" else (reveals.get(case) or {}).get("time")
        require(isinstance(start, (int, float)) and isinstance(retry.get("time"), (int, float)) and start <= retry["time"],
                f"{retry['kind']} in case {case} outside its input window")
        window = {"started": start, "ended": retry["time"]}
        reached = [r for r in receipts if inside(r, window) and (r.get("receipt") == "menu-perform" or
                                                                 (r.get("receipt") == "touch" and r.get("stage") == "began"))]
        require(not reached, f"{retry['kind']} in case {case} was not proven lost: the app received "
                             f"{sorted({r.get('receipt') for r in reached})} before it")


# Negative controls that must fail by name without a resend, with the words the failure must say. A tap
# beside the item never reaches the app: SpringBoard's open menu takes it and closes, so offset-pick fails
# as A2 (issue #142 stage 3; receipts in PR-3's text). `pick tap reached the app` stays as an A2 stage.
EXPECTED_FAILURES = {"drop-pick-twice": "pick lost twice", "offset-pick": "tap lost (class A2)",
                     "no-pid-no-state": "LAUNCH NO-PID not launched once more (class L)"}
# Class L negative controls on the one launch (issue #247): `no-pid:launch` must force one logged
# launch-retry named no-pid-app-running and pass; `no-pid-no-state:launch` must fail without one.
LAUNCH_FAULTS = ("no-pid", "no-pid-no-state")
LAUNCH = "launch"
NO_PID_APP_RUNNING = "no-pid-app-running"  # LaunchIssue.noPidAppRunning (smoke-qa/Tests/QAAx.swift)
# Negative controls on a choose() case; drop-tap names a fixture step instead.
CASE_FAULTS = ("skip-swipe", "drop-pick", "drop-open", *EXPECTED_FAILURES)
SHORT_DRAG = "short-drag"


def short_drag_retried(ui):
    """yebisu#242: the one window-mode record shows that the injected short drag left the window
    filling the screen with no app name, and that a later drag showed the name."""
    modes = [r for r in ui if r.get("kind") == "window-mode"]
    drags = modes[0].get("drags") if len(modes) == 1 else None
    if not isinstance(drags, list) or len(drags) < 2:
        return False
    first, last = drags[0], drags[-1]
    return (first.get("injectedShort") is True and first.get("fillsScreen") is True and first.get("nameShown") is False
            and not any(d.get("injectedShort") for d in drags[1:]) and last.get("nameShown") is True)


def valid_fault(spec):
    """True when spec is empty or a comma list of negative controls: `drop-tap:<step>`, and
    `skip-swipe:`, `drop-pick:`, `drop-open:`, `drop-pick-twice:` or `offset-pick:` with a choose() case,
    or `short-drag` (yebisu#242: the first window drag is too short to leave full screen)."""
    items = [item for item in spec.split(",") if item]
    def one(item):
        if item == SHORT_DRAG:
            return True
        kind, _, target = item.partition(":")
        if kind in LAUNCH_FAULTS:
            return target == LAUNCH
        return target in STEPS if kind == "drop-tap" else kind in CASE_FAULTS and target in CHOOSES
    return all(one(item) for item in items)


def expected_failure(spec):
    """(words, case) when spec is one control that must fail by name, else None. Such a control
    is run alone: with it, a test failure that names the case and the words is the pass."""
    items = [item for item in spec.split(",") if item]
    controls = [item.partition(":") for item in items if item.partition(":")[0] in EXPECTED_FAILURES]
    if not controls:
        return None
    require(len(items) == 1, f"an expected-failure control runs alone, got {spec!r}")
    kind, _, case = controls[0]
    return EXPECTED_FAILURES[kind], case


def failed_as_expected(spec, failure_text, ui):
    """Whether an expected-failure control (`expected_failure`) failed as it must: some failure line
    names its case and its words, and no pick was resent for drop-pick-twice more than once or for
    offset-pick at all."""
    words, case = expected_failure(spec)
    named = any(words in line and f"case {case}" in line for line in failure_text.splitlines())
    retried = "launch-retry" if case == LAUNCH else "pick-retry"
    resends = len([r for r in ui if r.get("kind") == retried and r.get("case") == case])
    return named and resends == (1 if words == EXPECTED_FAILURES["drop-pick-twice"] else 0)


def validate_faults(spec, ui):
    """Negative control: every injected fault must have forced exactly its logged retry, and nothing else may have retried.

    A fault that leaves no retry means the retry path was not exercised; a retry that
    no fault asked for is not hidden by this check (it stays in the verdict)."""
    retry_kinds = {"drop-tap": "fixture-retry", "skip-swipe": "reveal-retry", "drop-pick": "pick-retry", "drop-open": "open-retry",
                   "no-pid": "launch-retry"}
    for item in [item for item in spec.split(",") if item]:
        if item == SHORT_DRAG:
            require(short_drag_retried(ui), "injected fault short-drag did not force a logged second window drag")
            continue
        kind, _, target = item.partition(":")
        if kind in EXPECTED_FAILURES:
            continue  # judged by failed_as_expected
        key = "step" if kind == "drop-tap" else "case"
        found = [r for r in ui if r.get("kind") == retry_kinds[kind] and r.get(key) == target and r.get("injected") is True]
        require(len(found) == 1, f"injected fault {item} did not force exactly one logged retry")


def validate_gui_preconditions(records, ui, picks, prepares):
    """Name the missing precondition of a failed reveal (class B) or a lost tap (class A).

    These run before the setup-callback checks, whose generic 'missing/duplicated'
    would otherwise be the first symptom of a tap that never reached the app."""
    pre = [r for r in ui if r.get("kind") == "reveal-precondition"]
    require([r.get("case") for r in pre] == CHOOSES, "reveal precondition records missing/duplicated")
    for record in pre:
        require(isinstance(record.get("windowControlsPresent"), bool) and isinstance(record.get("appWindowFocused"), bool)
                and isinstance(record.get("appState"), str), "reveal precondition record incomplete")
    results = [r for r in ui if r.get("kind") == "reveal-result"]
    require([r.get("case") for r in results] == CHOOSES, "reveal result records missing/duplicated")
    for record, before in zip(results, pre):
        require(record.get("revealed") is True,
                f"menu bar not revealed (class B) in case {record.get('case')}: before the swipe Springboard Window Controls "
                f"present={before['windowControlsPresent']}, app state={before['appState']}, app window focused="
                f"{before['appWindowFocused']}, {before.get('secondsSinceMenuClosed')} s since the last menu closed")
    closes = [r for r in ui if r.get("kind") == "menu-close"]
    require([r.get("case") for r in closes] == CHOOSES, "menu close records missing/duplicated")
    for record in closes:
        require(record.get("closed") is True,
                f"OS menu still open after the {record.get('step')} tap in case {record.get('case')}: tap lost (class A); "
                f"{builds_near(records, record.get('tapTime'))}")
    require([r.get("case") for r in records if r.get("kind") == "p12-menu-build" and r.get("source") == "prepare"] == CASES,
            "menu build record missing for a prepared case")
    require(len([r for r in records if r.get("kind") == "p12-menu-build" and r.get("source") == "current"]) == 1,
            "menu build record missing for the current focus menu")
    entries = [r for r in records if r.get("kind") == "p12-menu-entry"]
    for pick in picks:
        require(pick.get("menuClosed") is True, f"OS menu not closed after the pick in case {pick.get('case')}: tap lost (class A)")
        if pick.get("tapped") and not any(entry.get("case") == pick.get("case") for entry in entries):
            stage = menu_stage(records, pick)
            if stage == "menu-deliver delivered":
                stage += ": the command reached GPUI; it stopped in GPUI or yebisu"
            raise ValueError(f"tap lost (class A2) in case {pick.get('case')}: {stage}; {builds_near(records, pick['started'])}")


def inside(record, window):
    time = record.get("time")
    return isinstance(time, (float, int)) and window["started"] <= time <= window["ended"]


def validate_observation(records, messages, ui, picks, prepares, receivers):
    # Strict joint ordering: callbacks must belong to the actual OS pick, and
    # every fixture tap must have zero command effect. Wall clocks must be
    # the same Simulator clock; missing/ambiguous observations fail closed.
    steps = STEPS
    controls = [r for r in ui if r.get("kind") == "fixture-control"]
    begins = [r for r in records if r.get("kind") == "p12-setup-begin"]
    ends = [r for r in records if r.get("kind") == "p12-setup-end"]
    require([r.get("step") for r in controls] == steps, "setup GUI steps missing")
    require([r.get("step") for r in begins] == steps == [r.get("step") for r in ends], "setup callbacks missing/duplicated")
    for control, begin, end in zip(controls, begins, ends):
        require(inside(begin, control) and inside(end, control) and begin["time"] <= end["time"], "setup callback outside actual control")
        require(end["before"] == end["after"] == [begin["state"]["sidebar"], begin["state"]["palette"]], "setup changed action state")
        require(end["state"]["connected"] and end["state"]["inbox_connected"] and end["state"]["handle_present"], "setup lost live transport")
        require(not any(inside(r, control) for r in records if r.get("kind") in ("action", "p12-menu-entry", "p12-menu-receiver")), "setup dispatched a command")
    windows = sorted(controls + picks, key=lambda r: r["started"])
    require(all(a["started"] < a["ended"] <= b["started"] for a, b in zip(windows, windows[1:])), "GUI windows overlap")
    entries = [r for r in records if r.get("kind") == "p12-menu-entry"]
    require(len(entries) == len(receivers), "entry/receipt cardinality mismatch")
    for case, pick, prepare in zip(CASES, picks, prepares):
        control = next(c for c in controls if c["step"] == case)
        require(control["ended"] < pick["started"], "pick preceded setup")
        state = prepare["state"]
        require(state["scene_active"] and state["focused"] and state["connected"] and state["inbox_connected"] and state["handle_present"], "invalid prepared scene/focus/transport")
        require(state["pending"] == (case in ("sidebar", "palette")), "invalid prepared pending state")
        require(state["original_host"] and state["original_pane"] and state["split_capability"], "invalid prepared context")
        observed = [r for r in entries if r.get("case") == case]
        finished = [r for r in receivers if r.get("case") == case]
        require(len(observed) == len(finished), "case entry/receipt mismatch")
        for entry, receipt in zip(observed, finished):
            require(inside(entry, pick) and inside(receipt, pick), "callback outside actual OS pick")
            for key in ("origin", "name", "state", "before", "mutated"):
                require(entry[key] == receipt[key], "entry/receipt context mismatch")
            require(receipt["guard"] in ("command_gate",), "central guard not observed")
            require(receipt["origin"] == "native_action", "keyless menu must enter native action")
            actual = receipt["state"]
            require(actual["generation"] == state["generation"] and actual["connected"] and actual["inbox_connected"] and actual["handle_present"], "pick lost same live context")
            if case in NEGATIVE:
                mutations = [r for r in records if r.get("kind") == "p12-mutated" and r.get("case") == case]
                require(len(mutations) == 1 and control["ended"] < mutations[0]["time"] < pick["started"], "stale mutation outside setup/pick")
                require(actual == mutations[0]["state"], "pick state differs from actual mutated state")
            else:
                require(actual == state, "pick state differs from prepared state")
        if case in NEGATIVE:
            mutations = [r for r in records if r.get("kind") == "p12-mutated" and r.get("case") == case]
            require(len(mutations) == 1, "stale fault not observed")
            actual = mutations[0]["state"]
            expected = dict(state)
            field = {"host":"original_host", "pane":"original_pane", "boot":"original_boot", "focus":"original_pane", "capability":"split_capability"}[case]
            expected[field] = False
            require(actual == expected, "wrong actual negative state")
    remote_picks = [picks[CASES.index(remote)] for remote, _ in HOLDS]
    actions = [r for r in records if r.get("kind") == "action"]
    require(len(actions) == len(HOLDS) and all(a.get("method") == "pane.split" and inside(a, pick) for a, pick in zip(actions, remote_picks)),
            "action outside the remote picks")
    starts = [m for m in messages if m.get("kind") == "p12-peer-start"]
    require(len(starts) == 1, "peer reconnected or start missing")
    transport = starts[0]["transport"]
    require(all(m.get("transport") == transport for m in messages), "mixed peer transports")
    require(not any(m.get("kind") == "p12-peer-exit" for m in messages), "peer exited before evidence collection")
    ingress = [m for m in messages if m.get("kind") == "p12-peer-ingress"]
    require([m["sequence"] for m in ingress] == list(range(1, len(ingress)+1)), "ingress missing/duplicated")
    for control in controls:
        during = [m for m in ingress if inside(m, control)]
        if control["step"] == "release":
            # Releasing the held split response makes the app re-lay out the new
            # pane and report geometry. That is the consequence of the release, not
            # a command: any request or input in this window still fails.
            require(all(m["message"] == {"geometry": True} for m in during), "release emitted non-geometry wire")
        elif control["step"] != "finish":
            require(not during, "setup emitted wire")
        else:
            require(all(m["message"] == {"input":["fifo_fence"]} for m in during), "finish emitted non-fence wire")
    requests = [m for m in ingress if "request_method" in m["message"]]
    require([m["message"]["request_method"] for m in requests] == ["pane.split"] * len(HOLDS), "remote RPC not exactly once per hold")
    require(all(inside(request, pick) for request, pick in zip(requests, remote_picks)), "wire request outside remote pick")
    gates = [m for m in messages if m.get("kind") == "p12-peer-gate"]
    require(len(gates) == len(HOLDS) and all(g["reason"] == "released" and g["sequence"] == r["sequence"] for g, r in zip(gates, requests)),
            "one-shot gate per hold not released")
    releases = [c for c in controls if c["step"] == "release"]
    for (_, local), request, gate, release in zip(HOLDS, requests, gates, releases):
        require(gate["time"] - request["time"] < COMMAND_TIMEOUT, "held request reached the client's 60 s COMMAND_TIMEOUT")
        pick = picks[CASES.index(local)]
        require(request["time"] < pick["started"] and pick["ended"] < gate["time"], "local case outside its own hold")
        require(inside(gate, release), "gate release not correlated")
    inputs = [m for m in ingress if "input" in m["message"]]
    require([m["message"]["input"] for m in inputs] == [["fifo_fence"]], "extra terminal input or missing FIFO fence")
    others = sorted({str(m["message"].get("message_kind", "?")) for m in ingress if m["message"].get("other_message")})
    require(not others, f"unexpected peer message: {others}")
    fence = inputs[0]
    require(inside(fence, controls[-1]) and fence["time"] > picks[-1]["ended"] and fence["sequence"] > requests[-1]["sequence"], "final receiving FIFO fence ordering")
    require(not any(m["sequence"] > fence["sequence"] and ("request_method" in m["message"] or "input" in m["message"]) for m in ingress), "wire after final FIFO fence")
