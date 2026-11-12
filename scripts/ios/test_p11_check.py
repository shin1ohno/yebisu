import copy
import importlib.util
from pathlib import Path
import unittest
spec=importlib.util.spec_from_file_location("p11",Path(__file__).with_name("p11-check.py"))
p11=importlib.util.module_from_spec(spec);spec.loader.exec_module(p11)
class P11Oracle(unittest.TestCase):
    def fixture(self):
        records=[{"kind":"p11-probe-start","scheduled":True,"action_blocked":True}, {"kind":"p11-probe-end","modal":True}]
        records += [{"kind":"p11-native-blocked"} for _ in range(4)]
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
if __name__=="__main__":unittest.main()
