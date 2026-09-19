#!/usr/bin/env python3
"""Declared, sealed 1..256 endpoint campaign through Zettai VCS bridges."""
import argparse
import concurrent.futures
import contextlib
import io
import itertools
import json
import platform
import random
import shutil
import subprocess
import time
import uuid
from pathlib import Path
from driver import *
from zettai import ZettaiSimulator,plan_zettai,route_proof
from experiment import workload,validate_run,seal,verify_seal,PROFILES
from correctness import run as correctness
from audit import audit_faults,mutations

ROOT=Path(__file__).resolve().parents[2]
COUNTS=[1,2,4,8,16,32,64,128,256]


def matrix():
    cells=[]
    for n,scaling,profile,enforce,schedule,repeat in itertools.product(COUNTS,['strong','weak'],PROFILES,[False,True],['serial','concurrent'],range(3)):
        c,b=PROFILES[profile]
        cells.append(dict(kind='scaling',n=n,scaling=scaling,profile=profile,enforce=enforce,schedule=schedule,repeat=repeat,compute_ns=c,bandwidth=b,record_ns=64))
    for scaling,profile,cost,repeat in itertools.product(['strong','weak'],PROFILES,[0,256],range(3)):
        c,b=PROFILES[profile]
        cells.append(dict(kind='sensitivity',n=256,scaling=scaling,profile=profile,enforce=True,schedule='concurrent',repeat=repeat,compute_ns=c,bandwidth=b,record_ns=cost))
    for n,repeat in itertools.product([16,64,256],range(3)):
        cells.append(dict(kind='correctness',n=n,repeat=repeat,enforce=True,compute_ns=8,bandwidth=16,record_ns=64))
    random.Random(20260920).shuffle(cells)
    return cells


def validate_topology(run,c,r):
    actual=json.loads((run/'topology.json').read_text());expected=plan_zettai(c['n'])
    assert len(actual)==len(expected)
    for a,e in zip(actual,expected):
        assert a==dict(e,pci_identity=0x0d928086,bar2_magic=0x43584c32)
    bridges=json.loads((run/'bridges.json').read_text())
    assert len(bridges)==2*((c['n']+63)//64)+(c['n']+7)//8
    assert len(r['route_proof'])==len(bridges)
    for b,row in zip(bridges,r['route_proof']):
        assert row['bridge']==b
        if b['kind']=='downstream':assert b['identity']==0xa1297a74
        affected=[n['tile'] for n in expected if n['switch']==b['switch'] and (b['kind']!='downstream' or n['dsp']==b['dsp'])]
        assert row['affected']==affected
        assert [x['tile'] for x in row['blocked']]==affected
        assert all(x['magic'] in (0,0xffffffff) for x in row['blocked'])
        assert row['restored']==[dict(tile=t,value=0x5a000000+t,magic=0x43584c32) for t in affected]
        others=[n['tile'] for n in expected if n['tile'] not in affected]
        assert row['peer']==(dict(tile=others[0],magic=0x43584c32) if others else None)
    trace=[json.loads(line) for line in (run/'qmp.jsonl').read_text().splitlines()]
    pending=None;binds=[];adds=[]
    for row in trace:
        msg=row['message']
        if row['direction']=='send':
            assert pending is None;pending=msg
        elif 'event' in msg or 'QMP' in msg:continue
        else:
            assert pending is not None and 'return' in msg and 'error' not in msg
            if pending['execute']=='zettai-bind-vppb':binds.append(pending['arguments'])
            if pending['execute']=='device_add':adds.append(pending['arguments'])
            pending=None
    assert pending is None
    assert binds==[dict(path=n['switch_id'],**{'vcs-id':0,'vppb-id':n['dsp'],'dsp-ppb-id':n['dsp']}) for n in expected if n['function']==0]
    assert [a['id'] for a in adds]==[n['device_id'] for n in expected if n['function']!=0]
    enumerated=json.loads((run/'query-pci.json').read_text())
    ids=[]
    def walk(x):
        if isinstance(x,dict):
            if 'qdev_id' in x and x['qdev_id'].startswith('t2_'):ids.append(x['qdev_id'])
            for v in x.values():walk(v)
        elif isinstance(x,list):
            for v in x:walk(v)
    walk(enumerated)
    assert sorted(ids)==sorted(n['device_id'] for n in expected)


def run_one(index,c,source,qemu,out):
    run=out/f'run-{index:03d}';start=time.monotonic()
    try:
        with contextlib.redirect_stdout(io.StringIO()):
            with ZettaiSimulator(source,qemu,run,c['n'],c) as sim:
                routes=route_proof(sim)
                result=correctness(sim) if c['kind']=='correctness' else workload(sim,c)
        result.update(config=c,run_id=str(uuid.uuid4()),route_proof=routes,host_elapsed_s=time.monotonic()-start)
        write_json(run/'result.json',result)
        validate_run(c,result);validate_topology(run,c,result)
        if c['kind']=='correctness':audit_faults(c,result)
        return index
    except BaseException as e:
        if run.exists():write_json(run/'failure.json',dict(config=c,error=repr(e)))
        raise


def validate(out):
    verify_seal(out);manifest=json.loads((out/'manifest.json').read_text());assert manifest['cells']==matrix()
    groups={};identities=set();checks=routes=rounds=0;negative=0
    for index,c in enumerate(manifest['cells']):
        run=out/f'run-{index:03d}';r=json.loads((run/'result.json').read_text())
        assert r['config']==c and r['run_id'] not in identities
        identities.add(r['run_id']);validate_run(c,r);validate_topology(run,c,r)
        routes+=len(r['route_proof'])
        if c['kind']=='correctness':
            audit_faults(c,r);checks+=len(r['checks']);continue
        rounds+=len(r['rounds'])
        if c['n']==256 and c['enforce'] and c['schedule']=='concurrent' and not negative:negative=mutations(c,r)
        key=tuple((k,v) for k,v in c.items() if k not in ['kind','repeat'])
        g=groups.setdefault(key,dict(config=dict(key),runs=[]))
        g['runs'].append(dict(run_id=r['run_id'],path=f'run-{index:03d}',duration_ns=r['rounds'][-1]['end_ns'],max_queue_ns=max(d['status']['queue_wait'] for row in r['rounds'] for d in row['devices'])))
    for g in groups.values():
        assert len(g['runs'])==3
        values=[x['duration_ns'] for x in g['runs']]
        assert min(values)==max(values)
        g.update(median_ns=values[0],min_ns=min(values),max_ns=max(values))
    assert rounds==2052 and len(identities)==693 and negative==10
    return dict(schema='slugarch.zettai256.v1',status='pass',fresh_processes=693,scaling_processes=684,
                correctness_processes=9,checked_rounds=rounds,checks=checks,bridge_disable_restore_checks=routes,
                injected_failures=45,rejected_offline_mutations=negative,max_endpoints=256,
                manifest_sha256=sha256(out/'manifest.json'),seal_sha256=sha256(out/'SHA256SUMS'),groups=list(groups.values()))


def main():
    ap=argparse.ArgumentParser();ap.add_argument('--out',type=Path,required=True);ap.add_argument('--summary',type=Path,required=True)
    ap.add_argument('--validate-only',action='store_true');ap.add_argument('--workers',type=int,default=4)
    ap.add_argument('--source',type=Path,default=Path('/root/CXLMemSim'))
    ap.add_argument('--qemu',type=Path,default=Path('/root/CXLMemSim/lib/qemu/build-slugarch/qemu-system-x86_64'))
    args=ap.parse_args();out=args.out.resolve()
    if not args.validate_only:
        if out.exists():raise RuntimeError('Refusing to overwrite campaign')
        out.mkdir(parents=True);source=out/'sources';source.mkdir();cells=matrix()
        for path in Path(__file__).parent.glob('*.py'):shutil.copy(path,source/path.name)
        for name in ['transport.py','campaign.py']:shutil.copy(ROOT/'targets/splash-type2'/name,source/name)
        qsrc=args.source/'lib/qemu'
        names=['hw/cxl/cxl_slugarch.c','include/hw/cxl/cxl_slugarch.h','hw/cxl/cxl_type2.c','include/hw/cxl/cxl_type2.h','hw/cxl/meson.build','hw/cxl/cxl_type2_coherency.c','hw/cxl/cxl-vcs-switch.c','include/hw/cxl/cxl_vcs_switch.h','hw/pci-bridge/cxl_upstream.c','hw/pci-bridge/cxl_downstream.c']
        for name in names:
            p=source/name;p.parent.mkdir(parents=True,exist_ok=True);shutil.copy(qsrc/name,p)
        (source/'qemu-working-tree.patch').write_bytes(subprocess.check_output(['git','-C',str(qsrc),'diff','HEAD']))
        shutil.copy(args.source/'qemu_integration/qtest_switch_offload.py',source/'qtest_switch_offload.py')
        write_json(out/'manifest.json',dict(schema='slugarch.zettai256.v1',cells=cells,workers=args.workers,
             qemu=str(args.qemu),qemu_sha256=sha256(args.qemu),qemu_source=git_info(qsrc),slugarch_source=git_info(ROOT),
             python=platform.python_version(),platform=platform.platform(),timing='QEMU virtual nanoseconds; same single shared output-link model',
             topology='1..4 Zettai switches, one USP each, <=8 DSP each, <=8 independent Type-2 PCI functions per DSP; dynamic bind function zero last'))
        with concurrent.futures.ProcessPoolExecutor(max_workers=args.workers) as pool:
            futures=[pool.submit(run_one,i,c,args.source,args.qemu,out) for i,c in enumerate(cells)]
            for done,f in enumerate(concurrent.futures.as_completed(futures),1):
                f.result()
                if done%16==0 or done==len(cells):print(f'{done}/{len(cells)} passed',flush=True)
        assert sha256(args.qemu)==json.loads((out/'manifest.json').read_text())['qemu_sha256']
        seal(out)
    summary=validate(out);write_json(args.summary,summary)
    print({k:v for k,v in summary.items() if k!='groups'})


if __name__=='__main__':main()
