"""Driver for the opt-in SlugArch endpoint MMIO interface."""
from __future__ import annotations
import hashlib
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'splash-type2'))
from transport import Simulator
from campaign import sha256, write_json, git_info

BASE = 0x200000
SRC, DST = 0x400000, 0x800000
MASK = (1 << 64) - 1
NAMES = ('kind', 'serial', 'epoch', 'sequence', 'job', 'src', 'dst', 'words',
         'scalar', 'policy', 'time_ns', 'committed')
REGS = dict(state=0x08, error=0x10, epoch=0x18, job_epoch=0x20, job=0x28,
            src=0x30, dst=0x38, words=0x40, scalar=0x48, release=0x50,
            fault=0x58, capacity=0x60, count=0x68, committed=0x70,
            finished=0x78, compute_start=0x80, compute_end=0x88,
            link_start=0x90, link_end=0x98, queue_wait=0xa0,
            doorbell=0xa8, rejected=0xb0, policy=0xb8, enforce=0xc0,
            record_ns=0xc8, compute_ns=0xd0, bandwidth=0xd8,
            link_ns=0xe0, requests=0xe8, completions=0xf0, failures=0xf8)


def oracle(data, scalar):
    values = struct.unpack(f'<{len(data)//8}Q', data)
    return struct.pack(f'<{len(values)}Q', *[(3*x+scalar+i)&MASK for i,x in enumerate(values)])


def inputs(tile, words):
    return hashlib.shake_256(f'slugarch-endpoint-v1/{tile}/{words}'.encode()).digest(words*8)


class Endpoint:
    def __init__(self, device):
        self.d = device
        assert self.read(0) == 0x534c554741524301

    def read(self, reg):
        return self.d.qt.readq(self.d.bar2 + BASE + (REGS[reg] if isinstance(reg,str) else reg))

    def write(self, reg, value):
        self.d.qt.writeq(self.d.bar2 + BASE + (REGS[reg] if isinstance(reg,str) else reg), value)

    def snapshot(self):
        return {k:self.read(k) for k in REGS if k != 'doorbell'}

    def records(self, failure=False):
        size = 128 if failure else self.read('count')*128
        data = self.d.read_bytes(self.d.bar2+BASE+(0x800 if failure else 0x1000),size) if size else b''
        result=[]
        for offset in range(0,len(data),128):
            raw=data[offset:offset+128]
            r=dict(zip(NAMES,struct.unpack('<12Q',raw[:96])))
            r.update(digest=raw[96:].hex(), raw_hex=raw.hex())
            result.append(r)
        return result

    def submit(self, words, scalar, release, epoch=None, fault=0, src=SRC, dst=DST, job=1):
        values=dict(job_epoch=self.read('epoch') if epoch is None else epoch,
                    job=job,src=src,dst=dst,words=words,scalar=scalar,
                    release=release,fault=fault)
        for name,value in values.items(): self.write(name,value)
        self.write('doorbell',1)


def now(sim):
    return sim.qt.readq(sim.devices[0].bar2 + BASE + 0x100)


def drain(sim, endpoints):
    steps=[]
    while any(e.read('state')==1 for e in endpoints):
        steps.append(int(sim.qt.cmd('clock_step')[1]))
        if len(steps)>1000: raise RuntimeError('endpoint did not complete')
    return steps


def options(config):
    return (f"slugarch=on,slugarch-enforce={'on' if config['enforce'] else 'off'},"
            f"slugarch-compute-ns={config['compute_ns']},slugarch-record-ns={config['record_ns']},"
            f"slugarch-bandwidth={config['bandwidth']},slugarch-link-ns=200")


def schedule_oracle(jobs, config):
    """Resource oracle from declared arrivals and work, not measured timestamps.

    Equal compute deadlines follow submission order in this FIFO experiment.
    Propagation is pipelined and does not hold link serialization capacity.
    """
    r=config['record_ns'] if config['enforce'] else 0
    expected={}
    free=0
    pending=sorted(enumerate(jobs),key=lambda ij:(ij[1]['release']+r+ij[1]['words']*config['compute_ns'],ij[0]))
    for _,j in pending:
        start=j['release']+r
        compute_end=start+j['words']*config['compute_ns']
        link_start=max(compute_end,free)
        free=link_start+(j['words']*8+config['bandwidth']-1)//config['bandwidth']
        expected[j['tile']]=dict(compute_start=start,compute_end=compute_end,
                link_start=link_start,link_end=free+200,queue_wait=link_start-compute_end,
                finished=free+200+r)
    return expected


def validate_records(records, tile, epoch, job, input_data, output_data, scalar, release, finished):
    assert len(records)==2
    for index,(r,data,kind,commit,timestamp) in enumerate(zip(records,[input_data,output_data],[1,2],[0,1],[release,finished])):
        assert r['kind']==kind and r['serial']==201+tile and r['epoch']==epoch
        assert r['sequence']==index+1 and r['job']==job and r['src']==SRC and r['dst']==DST
        assert r['words']==len(data)//8 and r['scalar']==scalar and r['policy']==0x534c554700000001
        assert r['time_ns']==timestamp and r['committed']==commit
        assert r['digest']==hashlib.sha256(data).hexdigest()
        assert bytes.fromhex(r['raw_hex'])==struct.pack('<12Q',*(r[n] for n in NAMES))+bytes.fromhex(r['digest'])
