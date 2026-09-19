#!/usr/bin/env python3
"""Endpoint gate and concurrent virtual-time campaign; retains all declared cells."""
from __future__ import annotations
import argparse
import contextlib
import io
import itertools
import json
import platform
import random
import shutil
import statistics
import subprocess
import uuid
from pathlib import Path
from driver import *
from correctness import run as correctness

ROOT=Path(__file__).resolve().parents[2]
PROFILES={'compute':(32,32),'balanced':(8,16),'link':(1,2)}


def matrix():
    cells=[]
    for n,scaling,profile,enforce,schedule,repeat in itertools.product(
            [1,2,4,8],['strong','weak'],PROFILES,[False,True],['serial','concurrent'],range(3)):
        c,b=PROFILES[profile]
        cells.append(dict(kind='scaling',n=n,scaling=scaling,profile=profile,enforce=enforce,
                          schedule=schedule,repeat=repeat,compute_ns=c,bandwidth=b,record_ns=64))
    for scaling,profile,r,repeat in itertools.product(['strong','weak'],PROFILES,[0,256],range(3)):
        c,b=PROFILES[profile]
        cells.append(dict(kind='sensitivity',n=8,scaling=scaling,profile=profile,enforce=True,
                          schedule='concurrent',repeat=repeat,compute_ns=c,bandwidth=b,record_ns=r))
    for n,repeat in itertools.product([1,2,8],range(3)):
        cells.append(dict(kind='correctness',n=n,repeat=repeat,enforce=True,compute_ns=8,bandwidth=16,record_ns=64))
    for n in [1,2,8]: cells.append(dict(kind='legacy',n=n,repeat=0))
    random.Random(20260919).shuffle(cells)
    return cells


def workload(sim,c):
    es=[Endpoint(d) for d in sim.devices]
    words=4096//c['n'] if c['scaling']=='strong' else 4096
    scalar=1
    rounds=[]
    for epoch in range(2,5):
        before=now(sim)
        order=list(range(c['n']))
        random.Random(c['repeat']*101+epoch).shuffle(order)
        for e in es:
            e.write('epoch',epoch)
            e.d.publish(SRC,inputs(e.d.tile,words))
            e.d.publish(DST,b'\xaa'*(words*8))
        jobs=[];steps=[]
        for tile in order:
            e=es[tile];release=now(sim)
            e.submit(words,scalar,release,job=epoch)
            assert e.read('state')==1
            assert e.read('count')==int(c['enforce'])
            assert e.d.host_read(DST,8)==b'\xaa'*8
            jobs.append(dict(tile=tile,release=release,words=words))
            if c['schedule']=='serial':steps+=drain(sim,[e])
        if c['schedule']=='concurrent':steps=drain(sim,es)
        expected=schedule_oracle(jobs,c)
        result=[];total=0
        for e in es:
            tile=e.d.tile;s=e.snapshot();out=e.d.host_read(DST,words*8)
            want=oracle(inputs(tile,words),scalar)
            assert out==want and s['state']==2 and s['committed']==1 and s['error']==0
            for key,value in expected[tile].items(): assert s[key]==value,(key,s,expected[tile])
            records=e.records()
            release=next(j['release'] for j in jobs if j['tile']==tile)
            if c['enforce']:validate_records(records,tile,epoch,epoch,inputs(tile,words),out,scalar,release,s['finished'])
            else:assert records==[]
            total=(total+sum(struct.unpack(f'<{words}Q',out)))&MASK
            result.append(dict(tile=tile,status=s,records=records,output_hex=out.hex()))
        rounds.append(dict(epoch=epoch,scalar=scalar,start_ns=before,end_ns=now(sim),jobs=jobs,
                           clock_steps=steps,reduction=total,devices=result))
        scalar=total
    return dict(status='pass',rounds=rounds)


def seal(out):
    files=sorted(p for p in out.rglob('*') if p.is_file() and p.name!='SHA256SUMS')
    (out/'SHA256SUMS').write_text(''.join(f'{sha256(p)}  {p.relative_to(out)}\n' for p in files))


def verify_seal(out):
    expected={}
    for line in (out/'SHA256SUMS').read_text().splitlines():
        digest,name=line.split('  ',1)
        assert name not in expected and not Path(name).is_absolute() and '..' not in Path(name).parts
        expected[name]=digest
    assert set(expected)=={str(p.relative_to(out)) for p in out.rglob('*') if p.is_file() and p.name!='SHA256SUMS'}
    for name,digest in expected.items():assert sha256(out/name)==digest,name


def validate_run(c,r):
    assert r['status']=='pass'
    if c['kind'] in ('correctness','legacy'):
        for check in r['checks']:assert check['actual']==check['expected']
        return
    assert len(r['rounds'])==3
    scalar=1;last_end=0
    for epoch,row in zip(range(2,5),r['rounds']):
        assert row['epoch']==epoch and row['scalar']==scalar and row['start_ns']==last_end
        words=4096//c['n'] if c['scaling']=='strong' else 4096
        jobs=row['jobs']
        assert len(jobs)==c['n'] and sorted(j['tile'] for j in jobs)==list(range(c['n']))
        previous=row['start_ns']
        for j in jobs:
            assert j['words']==words
            assert j['release']==(row['start_ns'] if c['schedule']=='concurrent' else previous)
            rr=c['record_ns'] if c['enforce'] else 0
            previous=j['release']+2*rr+words*c['compute_ns']+(words*8+c['bandwidth']-1)//c['bandwidth']+200
        expected=schedule_oracle(jobs,c)
        assert len(row['devices'])==c['n'] and [d['tile'] for d in row['devices']]==list(range(c['n']))
        reduction=0
        for d in row['devices']:
            t=d['tile'];s=d['status'];want=oracle(inputs(t,words),scalar)
            assert bytes.fromhex(d['output_hex'])==want
            assert s['state']==2 and s['error']==0 and s['committed']==1
            assert s['epoch']==epoch and s['words']==words and s['scalar']==scalar
            assert s['compute_ns']==c['compute_ns'] and s['bandwidth']==c['bandwidth'] and s['record_ns']==c['record_ns']
            assert s['enforce']==int(c['enforce']) and s['count']==2*int(c['enforce'])
            for k,v in expected[t].items():assert s[k]==v
            if c['enforce']:
                release=next(j['release'] for j in jobs if j['tile']==t)
                validate_records(d['records'],t,epoch,epoch,inputs(t,words),want,scalar,release,s['finished'])
            else:assert d['records']==[]
            reduction=(reduction+sum(struct.unpack(f'<{words}Q',want)))&MASK
        assert row['reduction']==reduction
        assert row['end_ns']==max(e['finished'] for e in expected.values())
        assert row['clock_steps']==sorted(set(row['clock_steps'])) and row['clock_steps'][-1]==row['end_ns']
        scalar=reduction;last_end=row['end_ns']


def validate(out):
    verify_seal(out)
    m=json.loads((out/'manifest.json').read_text())
    assert m['cells']==matrix() and len(m['cells'])==336
    groups={};checks=0;uuids=set();faults=0
    for index,c in enumerate(m['cells']):
        r=json.loads((out/f'run-{index:03d}'/'result.json').read_text())
        assert r['config']==c and r['run_id'] not in uuids
        uuids.add(r['run_id']);validate_run(c,r)
        if c['kind'] in ('correctness','legacy'):
            checks+=len(r['checks'])
            if c['kind']=='correctness':faults+=5
            continue
        key=tuple((k,v) for k,v in c.items() if k not in ['repeat','kind'])
        g=groups.setdefault(key,dict(config=dict(key),runs=[]))
        g['runs'].append(dict(run_id=r['run_id'],path=f'run-{index:03d}',duration_ns=r['rounds'][-1]['end_ns'],
                             max_queue_ns=max(d['status']['queue_wait'] for row in r['rounds'] for d in row['devices'])))
    for g in groups.values():
        assert len(g['runs'])==3
        g['median_ns']=statistics.median(x['duration_ns'] for x in g['runs'])
        g['min_ns']=min(x['duration_ns'] for x in g['runs']);g['max_ns']=max(x['duration_ns'] for x in g['runs'])
    return dict(schema='slugarch.endpoint.v1',status='pass',fresh_processes=336,scaling_processes=324,
                correctness_processes=9,legacy_processes=3,checked_rounds=972,checks=checks,
                injected_failures=faults,manifest_sha256=sha256(out/'manifest.json'),seal_sha256=sha256(out/'SHA256SUMS'),groups=list(groups.values()))


def main():
    ap=argparse.ArgumentParser();ap.add_argument('--out',type=Path,required=True)
    ap.add_argument('--summary',type=Path,required=True);ap.add_argument('--validate-only',action='store_true')
    ap.add_argument('--source',type=Path,default=Path('/root/CXLMemSim'))
    ap.add_argument('--qemu',type=Path,default=Path('/root/CXLMemSim/lib/qemu/build-slugarch/qemu-system-x86_64'))
    args=ap.parse_args();out=args.out.resolve()
    if not args.validate_only:
        if out.exists():raise RuntimeError('Refusing to overwrite campaign')
        out.mkdir(parents=True)
        cells=matrix();sources=out/'sources';sources.mkdir()
        for name in ['driver.py','correctness.py','experiment.py','test_endpoint.py']:
            shutil.copy(Path(__file__).with_name(name),sources/name)
        for name in ['transport.py','campaign.py']:
            shutil.copy(ROOT/'targets/splash-type2'/name,sources/name)
        qsrc=args.source/'lib/qemu'
        for name in ['hw/cxl/cxl_slugarch.c','include/hw/cxl/cxl_slugarch.h','hw/cxl/cxl_type2.c','include/hw/cxl/cxl_type2.h','hw/cxl/meson.build','hw/cxl/cxl_type2_coherency.c']:
            target=sources/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy(qsrc/name,target)
        (sources/'qemu-working-tree.patch').write_bytes(subprocess.check_output(['git','-C',str(qsrc),'diff','HEAD']))
        write_json(out/'manifest.json',dict(schema='slugarch.endpoint.v1',cells=cells,
              qemu=str(args.qemu),qemu_sha256=sha256(args.qemu),qemu_source=git_info(qsrc),slugarch_source=git_info(ROOT),
              python=platform.python_version(),platform=platform.platform(),timing='QEMU virtual nanoseconds; uncalibrated parameter study',
              kernel='uint64 y[i] = 3*x[i]+scalar+i; inputs resident; three dependent rounds'))
        for index,c in enumerate(cells):
            run=out/f'run-{index:03d}'
            with contextlib.redirect_stdout(io.StringIO()):
                with Simulator(args.source,args.qemu,run,c['n'],'' if c['kind']=='legacy' else options(c)) as sim:
                    if c['kind']=='legacy':
                        from campaign import correctness as legacy
                        result=dict(status='pass',checks=legacy(sim))
                    elif c['kind']=='correctness':result=correctness(sim)
                    else:result=workload(sim,c)
            result.update(run_id=str(uuid.uuid4()),config=c)
            write_json(run/'result.json',result)
            if index%24==0:print(f'{index+1}/{len(cells)} passed',flush=True)
        assert sha256(args.qemu)==json.loads((out/'manifest.json').read_text())['qemu_sha256']
        seal(out)
    summary=validate(out);write_json(args.summary,summary)
    print({k:v for k,v in summary.items() if k!='groups'})


if __name__=='__main__':main()
