"""Each missing visual/receiving path must independently fail the oracle."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('oracle', Path(__file__).with_name('smoke_oracle.py'))
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


class SmokeOracleTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.positive = self.root / 'positive'
        self.positive.mkdir()
        stages, events = [], []
        for index, (stage, text) in enumerate(oracle.EXPECTED.items()):
            timestamp = 100 + index * 70
            stages.append({'stage':stage, 'file':stage+'.png', 'time':timestamp})
            rows = [bytearray(bytes.fromhex('2e3440ff')*100) for _ in range(100)]
            for name, rect, colors in [('text',[0,0,100,10],['eceff4']), ('quad',[0,20,100,10],['88c0d0']), ('image',[0,40,100,32],['ebcb8b','b48ead']), ('input',[0,80,100,10],['eceff4'])]:
                events.append({'kind':'paint','region':name,'bounds':rect,'viewport':[100,100], 'title':oracle.TITLE,'input':text,'marked':None,'ticks':index+1,'time':timestamp-0.1})
                x,y,w,h=rect
                for yy in range(y,y+h):
                    for xx in range(x,x+w):
                        color=colors[(xx//50)%len(colors)]
                        rows[yy][xx*4:xx*4+4]=bytes.fromhex(color+'ff')
            events.append({'kind':'input','input':text,'marked':None,'time':timestamp-0.2})
            oracle.write_png(self.positive/(stage+'.png'),100,100,4,rows)
        (self.positive/'stages.json').write_text(json.dumps(stages))
        (self.positive/'app.jsonl').write_text(''.join(json.dumps(e)+'\n' for e in events))

    def test_positive_checks_all_receiving_paths(self):
        oracle.validate(self.positive, verbose=False)

    def test_each_independent_fault_is_rejected_for_its_own_reason(self):
        for mode, reason in [('text','text region'), ('quad','quad region'), ('image','image region'), ('input','input result'), ('counter-background','counter did not advance across background'), ('counter-idle','counter did not advance across idle')]:
            with self.subTest(mode=mode):
                negative = self.root / mode
                oracle.negative_fixture(self.positive, negative, mode)
                with self.assertRaisesRegex(oracle.SmokeFailure, reason):
                    oracle.validate(negative, verbose=False)


if __name__ == '__main__':
    unittest.main()
