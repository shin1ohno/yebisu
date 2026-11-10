import importlib.util
from pathlib import Path
import unittest
spec = importlib.util.spec_from_file_location("native", Path(__file__).with_name("p8-native-check.py"))
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)

class NativeOracleTests(unittest.TestCase):
    def fixture(self):
        records, messages = [], []
        for i in range(24):
            records.extend([
                {"kind":"native-start", "case":i, "scheduled":True,
                 "command_control":i//6>=2, "terminal_priority":i//6%2==1,
                 "action":["paste:","copy:","cut:","selectAll:","physical-v","physical-c"][i%6]},
                {"kind":"native-result", "case":i, "preparing":False,
                 "clipboard":"日本語" if i%6 in (1,2) or (i%6 == 5 and i//6 != 3) else f"native-text-{i}"}])
            events = [{"Paste":f"native-start-{i}"}]
            if i%6 == 0 or (i%6 == 4 and i//6 != 3):
                events.append({"Paste":f"native-text-{i}"})
            if i%6 in (4,5) and i//6 == 3:
                events.extend({"Key":{"code":{"Char":"v" if i%6==4 else "c"},"modifiers":2,"kind":kind}} for kind in ["Press","Release"])
            events.append({"Paste":f"native-end-{i}"})
            messages.append({"ClientShellPaneInput":{"pane_id":"w1:p1","events":events}})
        return records, messages
    def test_expected(self):
        native.validate(*self.fixture())
    def test_ctrl_v_instead_of_paste(self):
        records, messages = self.fixture()
        messages[18]["ClientShellPaneInput"]["events"][1] = {"Key":{"code":{"Char":"v"},"modifiers":2}}
        with self.assertRaisesRegex(ValueError,"wire mismatch"):
            native.validate(records,messages)
    def test_copy_not_performed(self):
        records, messages = self.fixture()
        records[39]["clipboard"]="native-text-19"
        with self.assertRaisesRegex(ValueError,"clipboard mismatch"):
            native.validate(records,messages)
    def test_missing_selector(self):
        records, messages = self.fixture()
        records[0]["scheduled"]=False
        with self.assertRaisesRegex(ValueError,"selector or setting"):
            native.validate(records,messages)
