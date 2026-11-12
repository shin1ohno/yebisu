"""Joint installed-GUI, consumer and private-peer oracle. No model-only UI pass."""
CASES = ["remote", "sidebar", "palette", "host", "pane", "revision", "focus", "capability"]
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
    negative_routes = {}
    for case, pick, prepare in zip(CASES, picks, prepares):
        require(pick.get("owner") in ["com.apple.springboard", "com.shin1ohno.yebisu"], "wrong native owner")
        label = LABELS.get(case, "Split Right")
        require(pick.get("command") == label and pick.get("buttonLabel", "").split(",")[0] == label, "wrong native command label")
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
    requests, inputs = [], []
    fence_seen = False
    for message in messages:
        if "ClientShellEndpointRequest" in message:
            import json
            require(not fence_seen, "request after final FIFO fence")
            requests.append(json.loads(message["ClientShellEndpointRequest"]["request"])["method"])
        if "ClientShellPaneInput" in message:
            events = message["ClientShellPaneInput"]["events"]
            inputs.extend(events)
            fence_seen |= {"Paste": "p12-fence"} in events
        require("ClientShellPopupInput" not in message, "popup input leakage")
    require(requests == ["pane.split"], "remote RPC not exactly once")
    require(inputs == [{"Paste": "p12-fence"}], "extra terminal key/text wire or missing FIFO fence")
    require("p12-finished" in screenshots, "final screenshot missing")
    return {"installed_os_menu": "pass", "consumer_peer": "pass", "command_hold": "unverified",
            "physical_keyboard_ime": "unverified", "p11_p13_modal_integration": "unverified", "negative_routes": negative_routes}
