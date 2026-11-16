"""Joint installed-GUI, consumer and private-peer oracle. No model-only UI pass."""
import re

CASES = ["remote", "sidebar", "rearm", "palette", "host", "pane", "boot", "focus", "capability"]
NEGATIVE = CASES[4:]
LABELS = {"remote": "Split Right", "rearm": "Split Right", "sidebar": "Toggle Sidebar", "palette": "Command Palette"}
# Each remote pick's split is held for exactly one local case, then released.
HOLDS = [("remote", "sidebar"), ("rearm", "palette")]
# sheprd-client limits.rs COMMAND_TIMEOUT: a held request this old fails the link.
COMMAND_TIMEOUT = 60
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


def validate(records, messages, ui, screenshots):
    picks = [r for r in ui if r.get("kind") == "os-menu-pick"]
    require([r.get("case") for r in picks] == CASES, "actual OS menu cases missing/duplicated")
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
            "physical_keyboard_ime": "unverified", "p11_p13_modal_integration": "unverified", "negative_routes": negative_routes}


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
        if pick.get("tapped"):
            require(any(entry.get("case") == pick.get("case") for entry in entries),
                    f"tap lost (class A) in case {pick.get('case')}: the pick reached no native menu callback; "
                    f"{builds_near(records, pick['started'])}")


def inside(record, window):
    time = record.get("time")
    return isinstance(time, (float, int)) and window["started"] <= time <= window["ended"]


def validate_observation(records, messages, ui, picks, prepares, receivers):
    # Strict joint ordering: callbacks must belong to the actual OS pick, and
    # every fixture click must have zero command effect. Wall clocks must be
    # the same Simulator clock; missing/ambiguous observations fail closed.
    steps = [*[s for hold in HOLDS for s in (*hold, "release")],
             *[s for case in NEGATIVE for s in ((case, "current", "reset") if case == "focus" else (case, "reset"))], "finish"]
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
    require(not any(m["message"].get("other_message") for m in ingress), "unexpected peer message")
    fence = inputs[0]
    require(inside(fence, controls[-1]) and fence["time"] > picks[-1]["ended"] and fence["sequence"] > requests[-1]["sequence"], "final receiving FIFO fence ordering")
    require(not any(m["sequence"] > fence["sequence"] and ("request_method" in m["message"] or "input" in m["message"]) for m in ingress), "wire after final FIFO fence")
