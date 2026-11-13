"""Joint installed-GUI, consumer and private-peer oracle. No model-only UI pass."""
CASES = ["remote", "sidebar", "palette", "host", "pane", "boot", "focus", "capability"]
NEGATIVE = CASES[3:]
LABELS = {"remote": "Split Right", "sidebar": "Toggle Sidebar", "palette": "Command Palette"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def validate(records, messages, ui, screenshots):
    picks = [r for r in ui if r.get("kind") == "os-menu-pick"]
    require([r.get("case") for r in picks] == CASES, "actual OS menu cases missing/duplicated")
    prepares = [r for r in records if r.get("kind") == "p12-prepare"]
    require([r.get("case") for r in prepares] == CASES, "app prepare cases missing/duplicated")
    receivers = [r for r in records if r.get("kind") == "p12-menu-receiver"]
    require(all(r.get("case") in CASES for r in receivers), "unexpected native callback")
    validate_observation(records, messages, ui, picks, prepares, receivers)
    negative_routes = {}
    for case, pick, prepare in zip(CASES, picks, prepares):
        require(pick.get("owner") in ["com.apple.springboard", "com.shin1ohno.yebisu"], "wrong native owner")
        label = LABELS.get(case, "Split Right")
        require(pick.get("command") == label and pick.get("buttonLabel", "").split("　")[0].split(",")[0] == label, "wrong native command label")
        require(all(f"p12-{stage}-{case}" in screenshots for stage in ["menu", "before", "after"]), "real screenshot missing")
        matches = [r for r in prepare["catalog"] if r["label"] == label]
        require(len(matches) == 1 and matches[0]["enabled"], "prepared catalog mismatch")
        received = [r for r in receivers if r["case"] == case]
        if case not in NEGATIVE:
            require(pick.get("enabled") is True and pick.get("tapped") is True, "positive native item was not tapped")
            require(len(received) == 1 and received[0].get("accepted") is True, "positive dispatch not exactly once")
            result = received[0]
            require(result["name"] == matches[0]["name"] and result["pending"], "wrong action or pending gate lost")
            require(prepare["pending"] == (case != "remote"), "local case was not tested during pending RPC")
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
            if pick.get("tapped"):
                require(pick.get("enabled") is True and len(received) == 1, "stale enabled GUI pick lacks one receiver")
                negative_routes[case] = "consumer_rejected_actual_os_pick"
                result = received[0]
                require(result.get("accepted") is False and result.get("mutated") is True, "stale consumer guard accepted")
                require(result["name"] == matches[0]["name"] and result["before"] == result["after"] == before, "wrong stale receiver/state")
            else:
                require(pick.get("enabled") is False and not received, "untapped item is not native disabled evidence")
                negative_routes[case] = "native_gui_disabled_no_consumer_dispatch"
    require([r.get("released") for r in records if r.get("kind") == "p12-release"] == [True], "peer gate not explicitly released")
    require([r.get("enqueued") for r in records if r.get("kind") == "p12-finish"] == [True], "fixture fence not queued")
    require("p12-finished" in screenshots, "final screenshot missing")
    return {"installed_os_menu": "pass", "consumer_peer": "pass", "command_hold": "unverified",
            "physical_keyboard_ime": "unverified", "p11_p13_modal_integration": "unverified", "negative_routes": negative_routes}


def inside(record, window):
    time = record.get("time")
    return isinstance(time, (float, int)) and window["started"] <= time <= window["ended"]


def validate_observation(records, messages, ui, picks, prepares, receivers):
    # Strict joint ordering: callbacks must belong to the actual OS pick, and
    # every fixture click must have zero command effect. Wall clocks must be
    # the same Simulator clock; missing/ambiguous observations fail closed.
    steps = ["remote", "sidebar", "palette", "release",
             *[s for case in NEGATIVE for s in (case, "reset")], "finish"]
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
            field = {"host":"original_host", "pane":"original_pane", "boot":"original_boot", "focus":"focused", "capability":"split_capability"}[case]
            expected[field] = False
            require(actual == expected, "wrong actual negative state")
    actions = [r for r in records if r.get("kind") == "action"]
    require(len(actions) == 1 and actions[0].get("method") == "pane.split" and inside(actions[0], picks[0]), "action outside sole remote pick")
    starts = [m for m in messages if m.get("kind") == "p12-peer-start"]
    require(len(starts) == 1, "peer reconnected or start missing")
    transport = starts[0]["transport"]
    require(all(m.get("transport") == transport for m in messages), "mixed peer transports")
    require(not any(m.get("kind") == "p12-peer-exit" for m in messages), "peer exited before evidence collection")
    ingress = [m for m in messages if m.get("kind") == "p12-peer-ingress"]
    require([m["sequence"] for m in ingress] == list(range(1, len(ingress)+1)), "ingress missing/duplicated")
    for control in controls:
        during = [m for m in ingress if inside(m, control)]
        if control["step"] != "finish":
            require(not during, "setup emitted wire")
        else:
            require(all(m["message"] == {"input":["fifo_fence"]} for m in during), "finish emitted non-fence wire")
    requests = [m for m in ingress if "request_method" in m["message"]]
    require([m["message"]["request_method"] for m in requests] == ["pane.split"], "remote RPC not exactly once")
    require(inside(requests[0], picks[0]), "wire request outside remote pick")
    gates = [m for m in messages if m.get("kind") == "p12-peer-gate"]
    require(len(gates) == 1 and gates[0]["reason"] == "released" and gates[0]["sequence"] == requests[0]["sequence"], "one-shot gate not released")
    require(requests[0]["time"] < gates[0]["time"] and inside(gates[0], controls[3]), "gate release not correlated")
    inputs = [m for m in ingress if "input" in m["message"]]
    require([m["message"]["input"] for m in inputs] == [["fifo_fence"]], "extra terminal input or missing FIFO fence")
    require(not any(m["message"].get("other_message") for m in ingress), "unexpected peer message")
    fence = inputs[0]
    require(inside(fence, controls[-1]) and fence["time"] > picks[-1]["ended"] and fence["sequence"] > requests[0]["sequence"], "final receiving FIFO fence ordering")
    require(not any(m["sequence"] > fence["sequence"] and ("request_method" in m["message"] or "input" in m["message"]) for m in ingress), "wire after final FIFO fence")
