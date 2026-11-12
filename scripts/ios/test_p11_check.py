import copy
import importlib.util
from pathlib import Path
import unittest
spec=importlib.util.spec_from_file_location("p11",Path(__file__).with_name("p11-check.py"))
p11=importlib.util.module_from_spec(spec);spec.loader.exec_module(p11)
class P11Oracle(unittest.TestCase):
    def fixture(self):
        records=[{"kind":"p11-probe-start","scheduled":True,"action_blocked":True}, {"kind":"p11-probe-end","modal":True}]
        records += [{"kind":"p11-native-blocked","action":a} for a in ["Copy","Paste","Cut","SelectAll"]]
        records += [{"kind":"terminal","font_size":15,"cell_size":[18,43]}, {"kind":"settings-loaded","font_size":None}, {"kind":"settings-loaded","font_size":15}]
        messages=[{"ClientShellPaneInput":{"pane_id":"w1:p1","events":[{"Paste":"p11-start"},{"Paste":"p11-end"}]}}, {"ClientShellResize":{"cell_width_px":18,"cell_height_px":43,"surface_size":{"cols":80,"rows":24},"pixel_mouse":False}}]
        return records,messages,{"terminal_font_size":15}
    def test_primary_shape(self):p11.validate(*self.fixture())
    def test_leak_negative(self):
        r,m,d=self.fixture();m[0]["ClientShellPaneInput"]["events"].insert(1,{"TextCommit":"P11_LEAK_COMMIT"})
        with self.assertRaisesRegex(ValueError,"wire leak"):p11.validate(r,m,d)
    def test_missing_native_callback_negative(self):
        r,m,d=self.fixture();r=[x for x in r if x.get("kind")!="p11-native-blocked"]
        with self.assertRaisesRegex(ValueError,"native selectors"):p11.validate(r,m,d)
    def test_wrong_resize_negative(self):
        r,m,d=self.fixture();m[1]["ClientShellResize"]["cell_height_px"]=40
        with self.assertRaisesRegex(ValueError,"receiving resize"):p11.validate(r,m,d)
    def test_false_relaunch_negative(self):
        r,m,d=self.fixture();r[-1]["font_size"]=14
        with self.assertRaisesRegex(ValueError,"relaunch"):p11.validate(r,m,d)
    def test_duplicate_action_is_not_four_routes(self):
        r,m,d=self.fixture();r[3]["action"]="Copy"
        with self.assertRaisesRegex(ValueError,"native selectors"):p11.validate(r,m,d)
    def test_scheduling_does_not_prove_sdk_dispatch(self):
        r,m,d=self.fixture();r=[x for x in r if x.get("kind")!="p11-native-blocked"]
        r.append({"kind":"p11-sdk-observation","supported":False})
        self.assertTrue(p11.observations(r,m,d)["fifo_markers_only"])
        self.assertFalse(p11.observations(r,m,d)["sdk_refusal_observable"])
        with self.assertRaisesRegex(ValueError,"unobservable"):p11.validate(r,m,d)
    def test_observation_detects_leak_without_dispatch_receipts(self):
        r,m,d=self.fixture();m[0]["ClientShellPaneInput"]["events"].append({"TextCommit":"leak"})
        self.assertFalse(p11.observations(r,m,d)["fifo_markers_only"])

class PositiveOracle(unittest.TestCase):
    def fixture(self):
        spec=importlib.util.spec_from_file_location("p8_fixture",Path(__file__).with_name("test_p8_native_check.py"))
        module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
        r,m=module.NativeOracleTests().fixture()
        r += [{"kind":"native-consumer","action":a,"modal":False} for a in ["Paste","Copy","Cut","SelectAll"]*4]
        return r,m
    def test_outside_receiving_shape(self):p11.validate_positive(*self.fixture())
    def test_no_consumer_not_scheduled_success(self):
        r,m=self.fixture();r=[x for x in r if x.get("kind")!="native-consumer"]
        with self.assertRaisesRegex(ValueError,"consumer receipts"):p11.validate_positive(r,m)
    def test_delayed_paste_outside_barrier_fails(self):
        r,m=self.fixture();e=m[0]["ClientShellPaneInput"]["events"];e.append(e.pop(1))
        with self.assertRaisesRegex(ValueError,"outside native action"):p11.validate_positive(r,m)
    def test_extra_complete_segment_fails(self):
        r,m=self.fixture()
        m.append({"ClientShellPaneInput":{"pane_id":"w1:p1","events":[{"Paste":"native-start-999"},{"TextCommit":"leak"},{"Paste":"native-end-999"}]}})
        with self.assertRaisesRegex(ValueError,"marker sequence"):p11.validate_positive(r,m)
    def test_wrong_pane_fails(self):
        r,m=self.fixture();m[0]["ClientShellPaneInput"]["pane_id"]="w1:p2"
        with self.assertRaisesRegex(ValueError,"wrong receiving pane"):p11.validate_positive(r,m)

if __name__=="__main__":unittest.main()
