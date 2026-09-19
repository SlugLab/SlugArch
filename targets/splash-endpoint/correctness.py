"""Live endpoint fault, ordering, isolation, and recovery checks."""
from driver import *


def run(sim):
    es=[Endpoint(d) for d in sim.devices]
    rows=[]
    evidence=[]
    def check(name,actual,expected):
        rows.append(dict(case=name,actual=actual,expected=expected))
        assert actual==expected, rows[-1]
    e=es[-1]
    def fresh():
        for x in es:
            x.write('epoch',x.read('epoch')+1)
            x.write('capacity',1024)
            x.d.publish(SRC,inputs(x.d.tile,16))
            x.d.publish(DST,b'\xaa'*128)
    def capture(case):
        evidence.append(dict(case=case, endpoints=[dict(tile=x.d.tile,status=x.snapshot(),records=x.records(),failure=x.records(True),output=x.d.host_read(DST,128).hex()) for x in es]))
    for case,fault,capacity,epoch_delta,dst,error in [
            ('request_failure',1,1024,0,DST,4),
            ('completion_failure',2,1024,0,DST,5),
            ('log_capacity',0,1,0,DST,3),
            ('stale_epoch',0,1024,-1,DST,1),
            ('invalid_range',0,1024,0,(1<<64)-8,2)]:
        fresh();e.write('capacity',capacity)
        e.submit(16,7,now(sim),epoch=e.read('epoch')+epoch_delta,fault=fault,dst=dst)
        for x in es[:-1]: x.submit(16,7,now(sim))
        drain(sim,es)
        post=case=='completion_failure'
        check(case+'_state',e.read('state'),4 if post else 3)
        check(case+'_error',e.read('error'),error)
        check(case+'_commit',e.read('committed'),int(post))
        check(case+'_output',e.d.host_read(DST,128).hex(),oracle(inputs(e.d.tile,16),7).hex() if post else 'aa'*128)
        check(case+'_count',e.read('count'),1 if post else 0)
        check(case+'_diagnostic',e.records(True)[0]['committed'],int(post))
        old=e.read('rejected');e.write('doorbell',1)
        check(case+'_sticky',e.read('rejected'),old+1)
        for x in es[:-1]:
            check(case+f'_peer_{x.d.tile}',x.read('state'),2)
            check(case+f'_peer_output_{x.d.tile}',x.d.host_read(DST,128).hex(),oracle(inputs(x.d.tile,16),7).hex())
        capture(case)
        # A new epoch must recover; the caller deliberately acknowledges the fault.
        fresh()
        for x in es: x.submit(16,11,now(sim))
        drain(sim,es)
        for x in es: check(case+f'_recovery_{x.d.tile}',x.d.host_read(DST,128).hex(),oracle(inputs(x.d.tile,16),11).hex())
        capture(case+'_recovery')
    fresh()
    t=now(sim);e.submit(16,17,t)
    check('busy_before_time',e.read('state'),1)
    check('request_precedes_output',e.read('count'),1)
    check('no_early_output',e.d.host_read(DST,128).hex(),'aa'*128)
    before=e.snapshot()
    for key,value in [('scalar',999),('epoch',999),('capacity',0),('doorbell',1)]: e.write(key,value)
    for key in ['scalar','epoch','capacity']:check('busy_freezes_'+key,e.read(key),before[key])
    check('busy_rejected_writes',e.read('rejected'),before['rejected']+4)
    e.d.publish(SRC,b'\xff'*128)
    end=e.read('compute_end');sim.qt.cmd(f'clock_set {end-1}')
    check('before_compute_deadline',e.d.host_read(DST,128).hex(),'aa'*128)
    sim.qt.cmd('clock_step') # Compute callback schedules output transfer.
    link=e.read('link_end');sim.qt.cmd(f'clock_set {link-1}')
    check('before_link_deadline',e.d.host_read(DST,128).hex(),'aa'*128)
    sim.qt.cmd('clock_step')
    check('output_committed_before_success',e.read('committed'),1)
    check('success_waits_for_record',e.read('state'),1)
    check('completion_not_yet_recorded',e.read('count'),1)
    drain(sim,es)
    check('input_snapshot',e.d.host_read(DST,128).hex(),oracle(inputs(e.d.tile,16),17).hex())
    check('complete_record',e.read('count'),2)
    validate_records(e.records(),e.d.tile,e.read('epoch'),1,inputs(e.d.tile,16),oracle(inputs(e.d.tile,16),17),17,t,e.read('finished'))
    original=e.records();e.write(0x1000,0xdeadbeef)
    check('log_read_only',e.records(),original)
    # Invalid descriptor size must not admit an operation.
    capture('ordering_snapshot_freeze')
    fresh();e.submit(0,1,now(sim))
    check('zero_work_rejected',e.read('state'),3)
    check('zero_work_error',e.read('error'),6)
    capture('zero_work')
    return dict(status='pass',checks=rows,evidence=evidence)
