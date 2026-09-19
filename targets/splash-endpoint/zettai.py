"""Real Zettai VCS bind + bridge forwarding for up to 256 Type-2 objects.

Each Zettai switch has one USP and up to eight DSPs. A DSP carries up to eight
independent Type-2 PCI functions. Four switches fit 256 objects in one PCI domain.
Nonzero functions are device_add first; function zero is bound last through
zettai-bind-vppb, following QEMU multifunction hotplug ordering.
"""
from __future__ import annotations
import json
import socket
import sys
import tempfile
import time
from pathlib import Path
from driver import Simulator, write_json
from transport import Device, MEM_SIZE


def plan_zettai(count):
    if not isinstance(count,int) or isinstance(count,bool) or not 1<=count<=256:
        raise ValueError('Zettai endpoint count must be 1..256')
    nodes=[]
    for tile in range(count):
        switch=tile//64;dsp=(tile//8)%8;fn=tile%8
        base=0x1000000000+tile*0x08000000
        nodes.append(dict(tile=tile,switch=switch,dsp=dsp,function=fn,
          serial=201+tile,bus=15+switch*10+dsp,bar2=base,bar4=base+MEM_SIZE,
          root_bus=12,root_dev=switch,usp_bus=13+switch*10,dsp_bus=14+switch*10,
          root_id=f'rp{switch}',usp_id=f'us{switch}',dsp_id=f'ds{switch}_{dsp}',
          switch_id=f'zettai{switch}',device_id=f't2_{tile}',layout='zettai'))
    return nodes


class QMP:
    def __init__(self,path,trace):
        self.sock=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM)
        self.sock.settimeout(30)
        for attempt in range(100):
            try:self.sock.connect(str(path));break
            except (FileNotFoundError,ConnectionRefusedError):time.sleep(.01)
        else:raise RuntimeError('QMP socket unavailable')
        self.file=self.sock.makefile('rwb');self.trace=trace.open('w')
        self.greeting=self.receive();assert 'QMP' in self.greeting
        self.execute('qmp_capabilities')

    def receive(self):
        line=self.file.readline()
        if not line:raise RuntimeError('QMP closed')
        value=json.loads(line)
        self.trace.write(json.dumps(dict(direction='receive',message=value),sort_keys=True)+'\n');self.trace.flush()
        return value

    def execute(self,command,arguments=None):
        value={'execute':command}
        if arguments is not None:value['arguments']=arguments
        self.trace.write(json.dumps(dict(direction='send',message=value),sort_keys=True)+'\n');self.trace.flush()
        self.file.write(json.dumps(value).encode()+b'\n');self.file.flush()
        while True:
            reply=self.receive()
            if 'event' in reply:continue
            if 'error' in reply:raise RuntimeError(reply)
            return reply['return']

    def close(self):
        self.file.close();self.sock.close();self.trace.close()


class ZettaiSimulator(Simulator):
    def __init__(self,source:Path,qemu:Path,out:Path,count:int,config:dict):
        self.topology=plan_zettai(count)
        sys.path.insert(0,str(source/'qemu_integration'))
        from qtest_switch_offload import launch_qemu,stop_process,pci_read,pci_write,configure_pci_bridge
        self.stop_process=stop_process
        self.tmp=tempfile.TemporaryDirectory(prefix='slugarch-zettai-')
        self.guard=socket.socket();self.guard.bind(('127.0.0.1',0))
        self.proc=self.qt=self.log=self.qmp=None
        out.mkdir(parents=True,exist_ok=True)
        sock=Path(self.tmp.name)/'qtest.sock';qmp=Path(self.tmp.name)/'qmp.sock'
        port=self.guard.getsockname()[1]
        args=[str(qemu),'-qtest',f'unix:{sock}','-qtest-log','/dev/null','-qmp',f'unix:{qmp},server=on,wait=off',
              '-display','none','-audio','none','-machine','q35,cxl=on','-m','256M','-nodefaults','-accel','qtest']
        switches=(count+63)//64
        for switch in range(switches):
            ports=min(8,(count-switch*64+7)//8)
            args+=['-object',f'zettai,id=zettai{switch},usp-ppbs=1,dsp-ppbs={ports},local-fm=true']
        args+=['-device','pxb-cxl,bus_nr=12,bus=pcie.0,id=cxl.0']
        self.device_args=[]
        for switch in range(switches):
            args+=['-device',f'cxl-rp,port={switch},bus=cxl.0,id=rp{switch},chassis=0,slot={switch+1},addr={switch:x}',
                   '-device',f'cxl-upstream,port=0,sn={10001+switch},bus=rp{switch},id=us{switch},addr=0.0,multifunction=on,vcs=zettai{switch},usppb=0']
            ports=min(8,(count-switch*64+7)//8)
            for dsp in range(ports):
                args+=['-device',f'cxl-downstream,port={dsp},bus=us{switch},id=ds{switch}_{dsp},chassis={switch+1},slot={dsp+1},addr={dsp:x}']
        for n in self.topology:
            d=dict(driver='cxl-type2',id=n['device_id'],bus=n['dsp_id'],addr=f"0.{n['function']}",
                   multifunction=True,sn=n['serial'],**{'gpu-mode':0,'cache-size':16<<20,'mem-size':MEM_SIZE,
                   'cxlmemsim-addr':'127.0.0.1','cxlmemsim-port':port,'coherency-enabled':True,
                   'slugarch':True,'slugarch-enforce':config['enforce'],'slugarch-compute-ns':config['compute_ns'],
                   'slugarch-record-ns':config['record_ns'],'slugarch-bandwidth':config['bandwidth'],'slugarch-link-ns':200})
            self.device_args.append(d)
            if n['function']==0:
                hidden={k:v for k,v in d.items() if k != 'driver'}
                hidden.update(vcs=n['switch_id'],dsppb=n['dsp'])
                def cli(v):return ('on' if v else 'off') if isinstance(v,bool) else str(v)
                args+=['-device','cxl-type2,'+','.join(f'{k}={cli(v)}' for k,v in hidden.items())]
        write_json(out/'command.json',args)
        try:
            self.proc,self.qt,self.log=launch_qemu(args,sock,out/'qemu.log')
            self.qt.sock.settimeout(60)
            self.qmp=QMP(qmp,out/'qmp.jsonl')
            # QEMU requires function zero last when hotplugging a multifunction slot.
            # Zettai binds it to the registered vPPB after other functions are staged.
            ordered=sorted(zip(self.topology,self.device_args),
                           key=lambda pair:(pair[0]["tile"]//8,pair[0]["function"]==0,pair[0]["function"]))
            for n,d in ordered:
                if n['function']==0:
                    self.qmp.execute('zettai-bind-vppb',dict(path=n['switch_id'],**{'vcs-id':0,'vppb-id':n['dsp'],'dsp-ppb-id':n['dsp']}))
                else:self.qmp.execute('device_add',d)
            self.bridges=[]
            for switch in range(switches):
                ns=[n for n in self.topology if n['switch']==switch]
                base=ns[0]['bar2'];limit=ns[-1]['bar2']+0x08000000;last=ns[-1]['bus']
                for kind,bus,dev,secondary in [('root',12,switch,ns[0]['usp_bus']),('upstream',ns[0]['usp_bus'],0,ns[0]['dsp_bus'])]:
                    configure_pci_bridge(self.qt,bus,dev,0,bus,secondary,last,base,limit-base)
                    self.bridges.append(dict(kind=kind,switch=switch,bus=bus,dev=dev,function=0,secondary=secondary,subordinate=last,base=base,size=limit-base,identity=pci_read(self.qt,bus,dev,0,0)))
                for n in ns:
                    if n['function']!=0:continue
                    functions=sum(x['dsp']==n['dsp'] for x in ns)
                    configure_pci_bridge(self.qt,n['dsp_bus'],n['dsp'],0,n['dsp_bus'],n['bus'],n['bus'],n['bar2'],functions*0x08000000)
                    # Runtime binding hotplugs into an initially empty, powered-off slot.
                    ctl=pci_read(self.qt,n['dsp_bus'],n['dsp'],0,0xa8)
                    pci_write(self.qt,n['dsp_bus'],n['dsp'],0,0xa8,ctl & ~0x400)
                    identity=pci_read(self.qt,n['dsp_bus'],n['dsp'],0,0)
                    if identity!=0xA1297A74:raise AssertionError(f'Not a Zettai DSP: {identity:#x}')
                    self.bridges.append(dict(kind='downstream',switch=switch,dsp=n['dsp'],bus=n['dsp_bus'],dev=n['dsp'],function=0,secondary=n['bus'],subordinate=n['bus'],base=n['bar2'],size=functions*0x08000000,identity=identity))
            self.devices=[]
            for n in self.topology:
                bus,fn=n['bus'],n['function']
                assert pci_read(self.qt,bus,0,fn,0)==0x0D928086
                for reg,address in [(0x18,n['bar2']),(0x20,n['bar4'])]:
                    pci_write(self.qt,bus,0,fn,reg,address&0xffffffff);pci_write(self.qt,bus,0,fn,reg+4,address>>32)
                    actual=(pci_read(self.qt,bus,0,fn,reg)&~15)|(pci_read(self.qt,bus,0,fn,reg+4)<<32)
                    assert actual==address,(n,actual,address)
                pci_write(self.qt,bus,0,fn,4,pci_read(self.qt,bus,0,fn,4)|6)
                assert self.qt.readl(n['bar2'])==0x43584C32,n
                n.update(pci_identity=0x0D928086,bar2_magic=0x43584C32)
                self.devices.append(Device(self.qt,n['tile'],n['bar2'],n['bar4']))
            write_json(out/'topology.json',self.topology);write_json(out/'bridges.json',self.bridges)
            write_json(out/'query-pci.json',self.qmp.execute('query-pci'))
        except BaseException:
            self.close();raise

    def close(self):
        if self.qmp:self.qmp.close();self.qmp=None
        super().close()


def route_proof(sim):
    """Disable each real bridge window; all descendants must become inaccessible.

    A blocked BAR4 write must not reach the endpoint. Other branches remain
    accessible. Restoring the bridge must expose the original sentinel again.
    """
    from qtest_switch_offload import pci_read,pci_write
    import struct
    rows=[]
    for d in sim.devices:d.publish(0x100000,struct.pack('<Q',0x5a000000+d.tile))
    for bridge in sim.bridges:
        affected=[n['tile'] for n in sim.topology if n['switch']==bridge['switch'] and
                  (bridge['kind']!='downstream' or n['dsp']==bridge['dsp'])]
        others=[n['tile'] for n in sim.topology if n['tile'] not in affected]
        bus,dev=bridge['bus'],bridge['dev']
        command=pci_read(sim.qt,bus,dev,0,4)
        pci_write(sim.qt,bus,dev,0,4,command & ~2)
        blocked=[]
        for tile in affected:
            d=sim.devices[tile];magic=sim.qt.readl(d.bar2)
            assert magic in (0,0xffffffff),(bridge,tile,magic)
            d.publish(0x100000,b'\xff'*8)
            blocked.append(dict(tile=tile,magic=magic))
        peer=None
        if others:
            d=sim.devices[others[0]];assert sim.qt.readl(d.bar2)==0x43584c32
            peer=dict(tile=d.tile,magic=sim.qt.readl(d.bar2))
        pci_write(sim.qt,bus,dev,0,4,command)
        restored=[]
        for tile in affected:
            d=sim.devices[tile];value=struct.unpack('<Q',d.host_read(0x100000,8))[0]
            assert value==0x5a000000+tile
            assert sim.qt.readl(d.bar2)==0x43584c32
            restored.append(dict(tile=tile,value=value,magic=0x43584c32))
        rows.append(dict(bridge=bridge,affected=affected,blocked=blocked,restored=restored,peer=peer))
    return rows
