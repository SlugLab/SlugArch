#!/usr/bin/env python3
"""Independent retained-evidence audit and adversarial validator checks."""
import argparse
import copy
import json
from pathlib import Path
from driver import *
from experiment import validate, validate_run


def audit_faults(c,r):
    assert len(r['evidence'])==12
    expected_names=[]
    cases={'request_failure':4,'completion_failure':5,'log_capacity':3,'stale_epoch':1,'invalid_range':2}
    for name in cases:expected_names.extend([name,name+'_recovery'])
    expected_names+=['ordering_snapshot_freeze','zero_work']
    assert [row['case'] for row in r['evidence']]==expected_names
    for row in r['evidence']:
        case=row['case'];assert len(row['endpoints'])==c['n']
        for tile,d in enumerate(row['endpoints']):
            assert d['tile']==tile
            s=d['status'];records=d['records']
            target=tile==c['n']-1
            failed=target and (case in cases or case=='zero_work')
            if failed:
                post=case=='completion_failure'
                assert s['state']==(4 if post else 3) and s['error']==cases.get(case,6)
                assert s['committed']==int(post) and len(records)==int(post)
                diag=d['failure'][0]
                assert diag['kind']==(4 if post else 3) and diag['committed']==int(post)
                assert diag['serial']==201+tile and diag['epoch']==s['epoch']
                want=oracle(inputs(tile,16),7) if post else b'\xaa'*128
                assert bytes.fromhex(d['output'])==want
                if post:
                    assert records[0]['kind']==1 and records[0]['committed']==0
                    assert records[0]['digest']==hashlib.sha256(inputs(tile,16)).hexdigest()
                    assert diag['digest']==hashlib.sha256(want).hexdigest()
                continue
            active=case in cases or case.endswith('_recovery') or (target and case=='ordering_snapshot_freeze')
            if not active:
                assert s['state']==0 and records==[] and d['output']=='aa'*128
                continue
            scalar=11 if case.endswith('_recovery') else (17 if case=='ordering_snapshot_freeze' else 7)
            want=oracle(inputs(tile,16),scalar)
            assert s['state']==2 and s['error']==0 and s['committed']==1
            assert bytes.fromhex(d['output'])==want
            validate_records(records,tile,s['epoch'],1,inputs(tile,16),want,scalar,records[0]['time_ns'],s['finished'])


def mutations(c,r):
    # These checks must fail even if a raw file's outer checksum is regenerated.
    edits=[lambda x:x['rounds'][0]['devices'][0].update(output_hex='00'),
           lambda x:x['rounds'][0]['devices'][0]['status'].update(finished=0),
           lambda x:x['rounds'][0]['devices'][0]['status'].update(queue_wait=999),
           lambda x:x['rounds'][0]['devices'][0]['status'].update(state=4),
           lambda x:x['rounds'][1].update(scalar=0),
           lambda x:x['rounds'][0]['jobs'][0].update(release=1),
           lambda x:x['rounds'][0]['devices'][0]['records'].pop(),
           lambda x:x['rounds'][0]['devices'][0]['records'][0].update(serial=999),
           lambda x:x['rounds'][0]['devices'][0]['records'][1].update(committed=0),
           lambda x:x['rounds'][0]['devices'][0]['records'][1].update(digest='00'*32)]
    for edit in edits:
        bad=copy.deepcopy(r);edit(bad)
        try:validate_run(c,bad)
        except (AssertionError,KeyError,ValueError,IndexError):pass
        else:raise AssertionError('corrupt evidence was accepted')
    return len(edits)


def main():
    ap=argparse.ArgumentParser();ap.add_argument('artifact',type=Path);args=ap.parse_args()
    summary=validate(args.artifact);m=json.loads((args.artifact/'manifest.json').read_text())
    faults=negative=0
    for i,c in enumerate(m['cells']):
        if c['kind']=='correctness':
            r=json.loads((args.artifact/f'run-{i:03d}/result.json').read_text());audit_faults(c,r);faults+=1
        elif c['kind']=='scaling' and c['enforce'] and c['n']==8 and c['schedule']=='concurrent' and not negative:
            r=json.loads((args.artifact/f'run-{i:03d}/result.json').read_text());negative=mutations(c,r)
    assert faults==9 and negative==10
    print(f'PASS: sealed matrix, {faults} independent fault audits, {negative} rejected evidence mutations')


if __name__=='__main__':main()
