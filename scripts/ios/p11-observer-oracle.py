#!/usr/bin/env python3
"""Strict SDK11 consumer oracle. Offline validation is never native evidence."""
import json
import re

SDK = '944ff7e21a43fbdacb0e78a5be4056c32029db73'
ACTIONS = ['Copy', 'Paste', 'Cut', 'SelectAll']

def exact(value, keys):
    if not isinstance(value, dict) or set(value) != set(keys):
        raise ValueError('unknown/missing schema fields')

def arm(value):
    exact(value, ['run','epoch','view','phase'])
    if not isinstance(value['run'],str) or not re.fullmatch('[0-9a-f]{32}',value['run']):
        raise ValueError('lossy run identity')
    for field in ['epoch','view','phase']:
        if type(value[field]) is not int or not 0 < value[field] < 2**64:
            raise ValueError('invalid arm identity')
    return tuple(value[k] for k in ['run','epoch','view','phase'])

def key(value):
    exact(value,['arm','invocation'])
    if type(value['invocation']) is not int or not 0<value['invocation']<2**64:
        raise ValueError('invalid invocation')
    return (*arm(value['arm']),value['invocation'])

def clean(row, kind):
    row=dict(row);row.pop('time',None)
    if row.pop('kind',None)!=kind:raise ValueError('wrong row type')
    return row

def validate(records, *, modal, drawing=False, disabled=False, allow_disabled=False):
    manifests=[clean(r,'p11-arm') for r in records if r.get('kind')=='p11-arm']
    phases=list(range(1,25)) if modal else [i+1 for i in range(24) if i%6<4]
    if len(manifests)!=len(phases):raise ValueError('missing arm manifest')
    identities={};run=None;view=None;epoch=0
    for manifest,phase in zip(manifests,phases):
        exact(manifest,['arm','action','responder','policy','disabled'])
        identity=arm(manifest['arm'])
        if run is None:run,view=identity[0],identity[2]
        if identity[0]!=run or identity[2]!=view or identity[1]<=epoch or identity[3]!=phase:
            raise ValueError('stale/reused arm or view')
        epoch=identity[1]
        i=phase-1
        action=ACTIONS[i%4] if modal else ['Paste','Copy','Cut','SelectAll'][i%6]
        origin=('Text' if i//4%2==0 else 'Drawing') if modal else ('Drawing' if drawing else 'Text')
        policy=(None if i//8==0 else [i//8==2]*4) if modal else ([True,True,True,False] if i%6 in (1,2) else [False,False,True,False])
        # Copy selection suppresses no paste availability; the actual predicate is frozen.
        if manifest['action']!=action or manifest['responder']!=origin or manifest['policy']!=policy or manifest['disabled'] is not disabled:
            raise ValueError('phase policy/action/origin mismatch')
        identities[identity]=(action,origin)
    pairs={};callbacks={};last_invocation=0;active=None
    for source in records:
        kind=source.get('kind')
        if kind=='p11-arm':
            active=arm(source['arm'])
        elif kind=='p11-sdk':
            r=clean(source,kind)
            exact(r,['schema','key','parent','action','responder','decision','event','handled','fallback'])
            k=key(r['key']);identity=k[:-1]
            if disabled:raise ValueError('disabled observer emitted records')
            if identity!=active:raise ValueError('record outside frozen arm interval')
            if r['schema']!=1 or identity not in identities or (r['action'],r['responder'])!=identities[identity] or r['parent'] is not None:
                raise ValueError('foreign or nested record')
            if r['event']=='Entry':
                if k in pairs or k[-1]<=last_invocation or any(r[f] is not None for f in ['decision','handled','fallback']):raise ValueError('duplicate/invalid entry')
                pairs[k]=[r];last_invocation=k[-1]
            else:
                if k not in pairs or len(pairs[k])!=1:raise ValueError('unmatched terminal')
                pairs[k].append(r)
        elif kind=='p11-consumer':
            r=clean(source,kind);exact(r,['key','route','edge']);k=key(r['key'])
            if k[:-1]!=active:raise ValueError('callback outside frozen arm interval')
            if k[:-1] not in identities or disabled:raise ValueError('foreign consumer key')
            callbacks.setdefault(k,[]).append((r['route'],r['edge']))
        elif kind=='p11-drain' and not disabled:
            completed=[pair for k,pair in pairs.items() if k[:-1]==active]
            if len(completed)!=1 or len(completed[0])!=2:raise ValueError('drain before exact arm pair')
    if not disabled:
        counts={identity:sum(k[:-1]==identity for k in pairs) for identity in identities}
        if any(count!=1 for count in counts.values()):raise ValueError('each arm requires exactly one native pair')
    if disabled and not allow_disabled and not pairs:raise ValueError("missing native pair")
    if disabled:
        if callbacks:raise ValueError('disabled callback key')
    elif len(pairs)!=len(phases):raise ValueError('missing native pair')
    for k,pair in pairs.items():
        if len(pair)!=2:raise ValueError('missing terminal outcome')
        terminal=pair[1];i=k[3]-1
        refused = (modal and i//8<2) or (not modal and i%6==3)
        decision = ('RefusedNoTextMode' if i//8==0 else 'RefusedPolicyDisallowed') if modal and refused else ('RefusedPolicyDisallowed' if refused else 'AllowedExplicitPolicy')
        if terminal['decision']!=decision or terminal['event']!=('Refused' if refused else 'Dispatched'):
            raise ValueError('wrong native terminal reason')
        expected=[] if refused else [('sheet-modal' if modal else 'terminal',edge) for edge in ['down','up']]
        if callbacks.pop(k,[])!=expected:raise ValueError('missing/duplicate actual consumer receipt')
        if refused:
            if terminal['handled'] is not None or terminal['fallback'] is not None:raise ValueError('invented refusal dispatch')
        elif terminal['handled'] is not True or terminal['fallback']!='NotEligible':raise ValueError('unhandled native dispatch or fallback')
    if callbacks:raise ValueError('orphan consumer receipt')
    statuses=[clean(r,r['kind']) for r in records if r.get('kind') in ['p11-drain','p11-status','p11-disabled']]
    if len(statuses)!=2*len(phases)+1:raise ValueError('missing status/drain/disable')
    sequence=0
    kinds=[r['kind'] for r in records if r.get('kind') in ['p11-drain','p11-status','p11-disabled']]
    if kinds!=['p11-drain','p11-status']*len(phases)+['p11-disabled']:raise ValueError('status order')
    for n,s in enumerate(statuses):
        exact(s,['schema','run','epoch','enabled','dropped','invalid','sticky','outstanding','reserved','queued','sequence'])
        final=n==len(statuses)-1
        if s['schema']!=1 or s['run']!=run or s['sequence']<=sequence or s['epoch']!=(epoch if final else manifests[n//2]['arm']['epoch']):raise ValueError('stale status identity')
        if any(s[f]!=0 for f in ['dropped','invalid','outstanding','reserved']) or s['sticky'] is not False:raise ValueError('invalid/drop/outstanding status')
        if s['queued']!=(0 if final or n%2 or disabled else 2) or s['enabled'] is not (not(disabled or final)):raise ValueError('queue/enable status')
        sequence=s['sequence']
    barriers=[r for r in records if r.get('kind')=='p11-peer-barrier']
    if [r.get('phase') for r in barriers]!=list(range(1,25)) or any(r.get('workers')!=0 or r.get('preparing') is not False for r in barriers):raise ValueError('missing actual async/receiving barrier')
    return run

def fifo(messages, modal):
    events=[]
    for message in messages:
        if 'ClientShellPaneInput' in message:
            body=message['ClientShellPaneInput']
            if body.get('pane_id')!='w1:p1':raise ValueError('wrong receiving target')
            events.extend(body['events'])
    if modal:
        expected=[{'Paste':f'p11-observer-{edge}-{i}'} for i in range(1,25) for edge in ['start','end']]
        if events!=expected:raise ValueError('modal wire leak or trailing input')
    return events

def positive(records,messages):
    starts=[r for r in records if r.get('kind')=='native-start']
    results=[r for r in records if r.get('kind')=='native-result']
    if [r.get('case') for r in starts]!=list(range(24)) or [r.get('case') for r in results]!=list(range(24)):raise ValueError('incomplete 24-case control')
    events=fifo(messages,False);expected=[]
    for i,(start,result) in enumerate(zip(starts,results)):
        if start['action']!=['paste:','copy:','cut:','selectAll:','physical-v','physical-c'][i%6] or start['command_control'] is not(i//6>=2) or start['terminal_priority'] is not(i//6%2==1):raise ValueError('wrong positive settings')
        if result.get('clipboard_matches') is not True or result.get('workers')!=0 or result.get('preparing') is not False:raise ValueError('async/clipboard completion missing')
        expected.append({'Paste':f'native-start-{i}'})
        if i//6==3 and i%6 in [4,5]:
            k='v' if i%6==4 else 'c'
            # Compare the meaningful wire key fields; reject extra events below.
            for edge in ['Press','Release']:
                expected.append(('key',k,edge))
        elif i%6 in [0,4]:expected.append({'Paste':f'native-text-{i}'})
        expected.append({'Paste':f'native-end-{i}'})
    if len(events)!=len(expected):raise ValueError('positive FIFO count/trailing input')
    for actual,want in zip(events,expected):
        if isinstance(want,tuple):
            k=actual.get('Key',{})
            if k.get('code')!={'Char':want[1]} or k.get('kind')!=want[2] or k.get('modifiers')!=2:raise ValueError('physical wire mismatch')
        elif actual!=want:raise ValueError('positive FIFO mismatch')
    controls=[r for r in records if r.get('kind')=='p11-physical-control']
    if [r.get('case') for r in controls]!=[i for i in range(24) if i%6>=4] or any(r.get('origin_none') is not True for r in controls):raise ValueError('physical origin contamination')


def settings(records,messages,disk):
    paints=[r for r in records if r.get('kind')=='terminal' and r.get('font_size')==15]
    loaded=[r for r in records if r.get('kind')=='settings-loaded']
    if not paints or disk.get('terminal_font_size')!=15 or len(loaded)<2 or loaded[-1].get('font_size')!=15:
        raise ValueError('font consumer/disk/relaunch mismatch')
    if not any([m['ClientShellResize']['cell_width_px'],m['ClientShellResize']['cell_height_px']]==p.get('cell_size') for m in messages if 'ClientShellResize' in m for p in paints):
        raise ValueError('missing receiving font resize')
    if any('key_line' in r for r in records):raise ValueError('key observation payload')
