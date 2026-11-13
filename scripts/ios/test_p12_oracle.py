"""Synthetic observation fixtures; never installed-UI or hardware evidence."""
import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("p12", Path(__file__).with_name("p12-oracle.py"))
p12 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p12)


def fixture():
    records, ui = [], []
    state = dict(original_boot=True, scene_active=True, focused=True, pending=False,
                 connected=True, inbox_connected=True, handle_present=True, generation=1,
                 original_host=True, original_pane=True, split_capability=True,
                 sidebar=False, palette=False)
    clock = 1

    def setup(step):
        nonlocal clock
        ui.append(dict(kind="fixture-control", step=step, started=clock, ended=clock+1))
        before = [state["sidebar"], state["palette"]]
        records.extend([dict(kind="p12-setup-begin", step=step, time=clock+.1, state=copy.deepcopy(state)),
                        dict(kind="p12-setup-end", step=step, time=clock+.2, state=copy.deepcopy(state), before=before, after=before)])
        clock += 2

    messages = [dict(kind="p12-peer-start", transport=1, time=0)]
    for case in p12.CASES:
        if case == "host":
            setup("release")
            records.append(dict(kind="p12-release", released=True, time=clock-1.5))
            messages.append(dict(kind="p12-peer-gate", transport=1, sequence=1, reason="released", time=clock-1.5))
            state["pending"] = False
        setup(case)
        prepared = copy.deepcopy(state)
        name = {"sidebar":"toggle_sidebar", "palette":"command_palette"}.get(case, "split_right")
        label = p12.LABELS.get(case, "Split Right")
        records.append(dict(kind="p12-prepare", case=case, time=clock-1.8, state=prepared,
                            sidebar=state["sidebar"], palette=state["palette"], pending=state["pending"],
                            catalog=[dict(name=name, label=label, enabled=True)]))
        negative = case in p12.NEGATIVE
        if negative:
            field = {"host":"original_host", "pane":"original_pane", "boot":"original_boot", "focus":"focused", "capability":"split_capability"}[case]
            state[field] = False
            records.append(dict(kind="p12-mutated", case=case, time=clock+.1, state=copy.deepcopy(state)))
            clock += 2
        before = [state["sidebar"], state["palette"]]
        entry = dict(kind="p12-menu-entry", case=case, name=name, time=clock+.1,
                     origin="native_action",
                     state=copy.deepcopy(state), before=before, mutated=negative)
        records.append(entry)
        if case == "remote":
            state["pending"] = True
            records.append(dict(kind="action", method="pane.split", time=clock+.2))
            messages.append(dict(kind="p12-peer-ingress", transport=1, sequence=1,
                                 message=dict(request_method="pane.split"), time=clock+.3))
        elif case in ("sidebar", "palette"):
            state[case] = not state[case]
        records.append(dict(entry, kind="p12-menu-receiver", time=clock+.4, guard="command_gate",
                            accepted=not negative, pending=state["pending"], after=[state["sidebar"], state["palette"]]))
        ui.append(dict(kind="os-menu-pick", case=case, started=clock, ended=clock+1,
                       owner="com.apple.springboard", command=label, buttonLabel=label+"　⌘D", enabled=True, tapped=True))
        clock += 2
        if negative:
            records.append(dict(kind="p12-reset", case=case, sidebar=state["sidebar"], palette=state["palette"], mutated=True))
            state = copy.deepcopy(prepared)
            setup("reset")
    setup("finish")
    records.append(dict(kind="p12-finish", enqueued=True, time=clock-1.5))
    messages.append(dict(kind="p12-peer-ingress", transport=1, sequence=2,
                         message=dict(input=["fifo_fence"]), time=clock-1.5))
    shots = {"p12-finished", *[f"p12-{stage}-{case}" for stage in ("menu", "before", "after") for case in p12.CASES]}
    return records, messages, ui, shots


def select(rows, kind, case=None):
    return next(r for r in rows if r.get("kind") == kind and (case is None or r.get("case") == case))


class P12OracleTests(unittest.TestCase):
    def rejected(self, edit, reason):
        data = fixture()
        edit(*data)
        with self.assertRaisesRegex(ValueError, reason):
            p12.validate(*data)

    def test_synthetic_native_route_keeps_hardware_unverified(self):
        result = p12.validate(*fixture())
        self.assertEqual(result["command_hold"], "unverified")
        self.assertEqual(result["p11_p13_modal_integration"], "unverified")

    def test_no_gui_is_not_callback_proof(self):
        self.rejected(lambda r,m,u,s: u.clear(), "actual OS menu")

    def test_duplicate_ingress_request(self):
        def edit(r,m,u,s):
            m.insert(-1, dict(m[1], sequence=2))
            m[-1]["sequence"] = 3
        self.rejected(edit, "RPC not exactly once")

    def test_key_or_text_leakage(self):
        self.rejected(lambda r,m,u,s: m[-1]["message"]["input"].insert(0, "unexpected_input"), "finish emitted non-fence wire")

    def test_missing_final_fence(self):
        self.rejected(lambda r,m,u,s: m.pop(), "missing FIFO fence")

    def test_stale_guard_acceptance(self):
        self.rejected(lambda r,m,u,s: select(r,"p12-menu-receiver","pane").update(accepted=True), "stale consumer guard accepted")

    def test_duplicate_local_callbacks(self):
        def edit(r,m,u,s):
            for kind in ("p12-menu-entry", "p12-menu-receiver"):
                r.append(copy.deepcopy(select(r,kind,"sidebar")))
        self.rejected(edit, "not exactly once")

    def test_missing_receipt(self):
        self.rejected(lambda r,m,u,s: r.remove(select(r,"p12-menu-receiver","remote")), "cardinality mismatch")

    def test_real_screenshot_required(self):
        self.rejected(lambda r,m,u,s: s.remove("p12-menu-focus"), "real screenshot missing")

    def test_unmutated_negative(self):
        self.rejected(lambda r,m,u,s: r.remove(select(r,"p12-mutated","host")), "stale mutation")

    def test_native_disabled_is_not_dispatch_proof(self):
        data = fixture()
        r,m,u,s = data
        for kind in ("p12-menu-entry", "p12-menu-receiver"):
            r.remove(select(r,kind,"focus"))
        select(u,"os-menu-pick","focus").update(tapped=False, enabled=False)
        self.assertEqual(p12.validate(*data)["negative_routes"]["focus"], "native_gui_disabled_no_consumer_dispatch")

    def test_key_route_never_fabricates_native_identity(self):
        def edit(r,m,u,s):
            for kind in ("p12-menu-entry", "p12-menu-receiver"):
                select(r,kind,"remote").update(origin="shortcut_key_down", captured_revision=None)
        self.rejected(edit, "keyless menu must enter native action")

    def test_current_revision_cannot_rescue_key_route(self):
        def edit(r,m,u,s):
            for kind in ("p12-menu-entry", "p12-menu-receiver"):
                entry = select(r,kind,"boot")
                entry.update(origin="shortcut_key_down", captured_revision=99)
        self.rejected(edit, "keyless menu must enter native action")

    def test_preparation_click_through(self):
        def edit(r,m,u,s):
            setup = next(x for x in u if x.get("step") == "host")
            r.append(dict(kind="action",method="pane.split",time=setup["started"]+.3))
        self.rejected(edit, "setup dispatched a command")

    def test_setup_geometry_is_wire_too(self):
        def edit(r,m,u,s):
            host = next(c for c in u if c.get("step") == "host")
            m.insert(-1, dict(kind="p12-peer-ingress", transport=1, sequence=2,
                             time=host["started"]+.5, message=dict(geometry=True)))
            m[-1]["sequence"] = 3
        self.rejected(edit, "setup emitted wire")

    def test_setup_state_delta(self):
        self.rejected(lambda r,m,u,s: select(r,"p12-setup-end").update(after=[True,False]), "setup changed action state")

    def test_false_connected_restore_does_not_prove_transport(self):
        self.rejected(lambda r,m,u,s: select(r,"p12-setup-end")["state"].update(inbox_connected=False), "lost live transport")

    def test_peer_gate_sender_exit(self):
        self.rejected(lambda r,m,u,s: m.append(dict(kind="p12-peer-exit",transport=1,reason="sender_disconnected")), "peer exited")

    def test_one_shot_gate_not_replayed(self):
        self.rejected(lambda r,m,u,s: m.append(copy.deepcopy(select(m,"p12-peer-gate"))), "one-shot gate")

    def test_reconnect_cannot_rescue_fence(self):
        self.rejected(lambda r,m,u,s: m[-1].update(transport=2), "mixed peer transports")

    def test_wrong_actual_negative_state(self):
        self.rejected(lambda r,m,u,s: select(r,"p12-mutated","host")["state"].update(original_host=True), "pick state differs")

    def test_no_behind_overlay_callback_as_gui_substitute(self):
        def edit(r,m,u,s):
            select(r,"p12-menu-entry","remote")["time"] = -1
        self.rejected(edit, "callback outside actual OS pick")


if __name__ == "__main__":
    unittest.main()
