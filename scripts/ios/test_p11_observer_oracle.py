"""Synthetic SDK-shaped oracle adversaries; not actual UIKit/serializer proof."""
import copy
import importlib.util
from pathlib import Path
import unittest
spec=importlib.util.spec_from_file_location('oracle',Path(__file__).with_name('p11-observer-oracle.py'))
o=importlib.util.module_from_spec(spec);spec.loader.exec_module(o)

def fixture(disabled=False):
    records=[];messages=[];run='0123456789abcdef0123456789abcdef';seq=0
    for p in range(1,25):
        a={'run':run,'epoch':p,'view':73,'phase':p};k={'arm':a,'invocation':p}
        action=['Copy','Paste','Cut','SelectAll'][(p-1)%4];origin='Text' if (p-1)//4%2==0 else 'Drawing'
        policy=None if p<=8 else [p>16]*4
        records.append({'kind':'p11-arm','arm':a,'action':action,'responder':origin,'policy':policy,'disabled':disabled})
        base={'kind':'p11-sdk','schema':1,'key':k,'parent':None,'action':action,'responder':origin,'handled':None,'fallback':None}
        if not disabled:
            records.append(dict(base,event='Entry',decision=None))
            if p>16:
                for edge in ['down','up']:records.append({'kind':'p11-consumer','key':k,'route':'sheet-modal','edge':edge})
            terminal=dict(base,event='Refused',decision='RefusedNoTextMode' if p<=8 else 'RefusedPolicyDisallowed')
            if p>16:terminal.update(event='Dispatched',decision='AllowedExplicitPolicy',handled=True,fallback='NotEligible')
            records.append(terminal)
        for kind,count in [('p11-drain',0 if disabled else 2),('p11-status',0)]:
            seq+=1
            records.append({'kind':kind,'schema':1,'run':run,'epoch':p,'enabled':not disabled,'dropped':0,'invalid':0,'sticky':False,'outstanding':0,'reserved':0,'queued':count,'sequence':seq})
        records.append({'kind':'p11-peer-barrier','phase':p,'workers':0,'preparing':False})
        for edge in ['start','end']:messages.append({'ClientShellPaneInput':{'pane_id':'w1:p1','events':[{'Paste':f'p11-observer-{edge}-{p}'}]}})
    records.append({'kind':'p11-disabled','schema':1,'run':run,'epoch':24,'enabled':False,'dropped':0,'invalid':0,'sticky':False,'outstanding':0,'reserved':0,'queued':0,'sequence':seq+1})
    return records,messages

class ObserverOracle(unittest.TestCase):
    def test_host_shaped_modal(self):
        records,messages=fixture();o.validate(records,modal=True);o.fifo(messages,True)
    def test_missing_entry_and_terminal(self):
        for event in ['Entry','Refused','Dispatched']:
            r,_=fixture();r.remove(next(x for x in r if x.get('event')==event))
            with self.assertRaises(ValueError):o.validate(r,modal=True)
    def test_real_callback_correlation_required(self):
        for field,value in [('route','terminal'),('edge','up')]:
            r,_=fixture();next(x for x in r if x['kind']=='p11-consumer')[field]=value
            with self.assertRaisesRegex(ValueError,'consumer receipt'):o.validate(r,modal=True)
    def test_no_phase_rescue_or_raw_pointer(self):
        for field,value in [('view',74),('phase',99),('run','0'*32)]:
            r,_=fixture();x=next(x for x in r if x['kind']=='p11-sdk');x['key']=copy.deepcopy(x['key']);x['key']['arm'][field]=value
            with self.assertRaisesRegex(ValueError,'foreign'):o.validate(r,modal=True)
        r,_=fixture();next(x for x in r if x['kind']=='p11-sdk')['pointer']=123
        with self.assertRaisesRegex(ValueError,'schema fields'):o.validate(r,modal=True)
    def test_reason_and_handled_are_not_fallback_success(self):
        for field,value in [('decision','RefusedNoTextFocus'),('handled',False),('fallback','Invoked')]:
            r,_=fixture();next(x for x in r if x.get('event')=='Dispatched')[field]=value
            with self.assertRaises(ValueError):o.validate(r,modal=True)
    def test_out_of_band_loss_and_predrain_count(self):
        for field,value in [('dropped',1),('invalid',1),('sticky',True),('outstanding',1),('reserved',1),('queued',0),('sequence',0)]:
            r,_=fixture();next(x for x in r if x['kind']=='p11-drain')[field]=value
            with self.assertRaises(ValueError):o.validate(r,modal=True)
    def test_disabled_intended_absent_pair_negative(self):
        r,_=fixture(True)
        with self.assertRaisesRegex(ValueError,'missing native pair'):o.validate(r,modal=True,disabled=True)
        o.validate(r,modal=True,disabled=True,allow_disabled=True)
    def test_receiver_tail_and_wrong_destination(self):
        _,m=fixture();m.append({'ClientShellPaneInput':{'pane_id':'w1:p1','events':[{'Paste':'leak'}]}})
        with self.assertRaisesRegex(ValueError,'trailing input'):o.fifo(m,True)
        _,m=fixture();m[0]['ClientShellPaneInput']['pane_id']='w1:p2'
        with self.assertRaisesRegex(ValueError,'target'):o.fifo(m,True)
    def test_worker_and_receiver_missing_completion(self):
        for field,value in [('workers',1),('preparing',True),('phase',99)]:
            r,_=fixture();next(x for x in r if x['kind']=='p11-peer-barrier')[field]=value
            with self.assertRaisesRegex(ValueError,'barrier'):o.validate(r,modal=True)
if __name__=='__main__':unittest.main()
