"""space-menu-check.py (#231): the receiving-side counts it judges after the Simulator run."""
import importlib.util
import json
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location('space_menu_check', Path(__file__).with_name('space-menu-check.py'))
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


def request(method, params):
    return {'ClientShellEndpointRequest': {'request': json.dumps({'method': method, 'params': params}),
                                           'request_id': '1'}}


def doc_set(result):
    return {'method': 'doc.set', 'version': 1, 'bytes': 10, 'result': result}


class SpaceMenuCheckTests(unittest.TestCase):
    def passing(self):
        peer = [request('workspace.close', {'workspace_id': 'w2'}),
                request('workspace.rename', {'workspace_id': 'w1', 'label': 'ship'}),
                {'ClientShellPaneInput': {'pane_id': 'w1:p1', 'events': []}}]
        wire = [doc_set('ok'), {'method': 'doc.get'}, doc_set('ok'), doc_set('ok')]
        return peer, wire

    def test_one_close_one_rename_three_writes_pass(self):
        peer, wire = self.passing()
        self.assertTrue(all(ok for ok, _ in CHECK.verdicts(peer, wire).values()))

    def test_a_second_close_fails(self):
        peer, wire = self.passing()
        peer.append(request('workspace.close', {'workspace_id': 'w2'}))
        self.assertFalse(CHECK.verdicts(peer, wire)['herdr1-close'][0])

    def test_a_close_of_another_space_fails(self):
        peer, wire = self.passing()
        peer[0] = request('workspace.close', {'workspace_id': 'w1'})
        self.assertFalse(CHECK.verdicts(peer, wire)['herdr1-close'][0])

    def test_no_rename_fails(self):
        peer, wire = self.passing()
        del peer[1]
        self.assertFalse(CHECK.verdicts(peer, wire)['herdr1-rename'][0])

    def test_a_missing_or_refused_write_fails(self):
        peer, wire = self.passing()
        self.assertFalse(CHECK.verdicts(peer, wire[:-1])['herdr2-doc-set'][0])
        wire[0] = doc_set('conflict')
        self.assertFalse(CHECK.verdicts(peer, wire)['herdr2-doc-set'][0])

    def test_malformed_records_are_skipped(self):
        peer = ['text', {'a': 1, 'b': 2}, {'ClientShellEndpointRequest': {'request': '{bad'}},
                {'ClientShellEndpointRequest': 'x'}]
        self.assertEqual(CHECK.peer_requests(peer), [])


if __name__ == '__main__':
    unittest.main()
