"""Multiple real Splash/CXLMemSim QEMU Type-2 objects in one qtest process.

Uses the existing simulated device-memory/BAR path, with no memory server,
guest OS, GPU execution, CFMWS routing, or endpoint SlugArch JIT implied.
"""
from __future__ import annotations

import json
import socket
import struct
import sys
import tempfile
from pathlib import Path

MEM_SIZE = 64 << 20
STAT_NAMES = (
    "snoop_hits", "snoop_misses", "coherency_requests", "back_invalidations",
    "writebacks", "evictions", "bias_flips", "device_bias_hits",
    "host_bias_hits", "upgrades", "downgrades", "directory_entries",
)


def plan_topology(count: int, layout: str = "auto") -> list[dict]:
    """Use multifunction slots to fit 256 independent objects in one PCI domain.

    The packed layout is an enumeration arrangement, not a model of 256
    physical links. The endpoint engine separately models one shared link.
    """
    if not isinstance(count, int) or isinstance(count, bool) or not 1 <= count <= 256:
        raise ValueError("endpoint count must be an integer from 1 through 256")
    if layout == "auto":
        layout = "legacy" if count <= 8 else "packed"
    if layout not in ("legacy", "packed") or (layout == "legacy" and count > 8):
        raise ValueError("legacy layout supports at most eight endpoints; use packed")
    nodes = []
    for tile in range(count):
        port, function = (tile, 0) if layout == "legacy" else divmod(tile, 8)
        # Keep the original low-address map for retained <=8-device experiments.
        # Start packed apertures at 64 GiB, above q35/CXL reserved low regions.
        base = (0x40000000 if layout == "legacy" else 0x1000000000) + tile * 0x08000000
        nodes.append(dict(tile=tile, port=port, function=function, bus=13+port,
                          serial=201+tile, bar2=base, bar4=base+MEM_SIZE,
                          layout=layout))
    return nodes


class Device:
    def __init__(self, qt, tile: int, bar2: int, bar4: int):
        self.qt, self.tile, self.bar2, self.bar4 = qt, tile, bar2, bar4

    def write_bytes(self, address: int, data: bytes):
        self.qt.cmd(f"write {address:#x} {len(data):#x} 0x{data.hex()}")

    def read_bytes(self, address: int, size: int) -> bytes:
        return bytes.fromhex(self.qt.cmd(f"read {address:#x} {size:#x}")[1].removeprefix("0x"))

    def command(self, opcode: int, *params: int, allow_error=False):
        self.write_bytes(self.bar2 + 0x40, struct.pack("<8Q", *(params + (0,) * (8 - len(params)))))
        self.qt.writel(self.bar2 + 0x10, opcode)
        status = self.qt.readl(self.bar2 + 0x14)
        result = self.qt.readl(self.bar2 + 0x18)
        if status != 3 or (result and not allow_error):
            raise RuntimeError(f"tile={self.tile} opcode={opcode:#x} status={status} result={result}")
        values = struct.unpack("<4Q", self.read_bytes(self.bar2 + 0x80, 32))
        return result, values

    def publish(self, offset: int, data: bytes):
        self.write_bytes(self.bar4 + offset, data)

    def host_read(self, offset: int, size: int) -> bytes:
        return self.read_bytes(self.bar4 + offset, size)

    def consume(self, offset: int, size: int) -> bytes:
        self.command(0x23, offset, size)
        return self.read_bytes(self.bar2 + 0x1000, size)

    def device_write(self, offset: int, data: bytes):
        self.write_bytes(self.bar2 + 0x1000, data)
        self.command(0x22, offset, len(data))

    def stats(self) -> dict:
        _, values = self.command(0xB0)
        values += struct.unpack("<8Q", self.read_bytes(self.bar2 + 0x1000, 64))
        return dict(zip(STAT_NAMES, values))

    def reset_stats(self):
        self.command(0xB1)
        self.command(0xF2)

    def modeled_ns(self) -> int:
        return self.command(0xF1)[1][0]


class Simulator:
    def __init__(self, source: Path, qemu: Path, out: Path, count: int,
                 device_options: str = "", layout: str = "auto"):
        self.topology = plan_topology(count, layout)
        sys.path.insert(0, str(source / "qemu_integration"))
        from qtest_switch_offload import configure_pci_bridge, launch_qemu, pci_read, pci_write, stop_process
        self.stop_process = stop_process
        self.tmp = tempfile.TemporaryDirectory(prefix="slugarch-splash-")
        # Reserve an unlistened local port to make the selected local-memory
        # substrate explicit and prevent connection to another user's server.
        self.guard = socket.socket()
        self.guard.bind(("127.0.0.1", 0))
        self.proc = self.qt = self.log = None
        port = self.guard.getsockname()[1]
        sock = Path(self.tmp.name) / "qtest.sock"
        out.mkdir(parents=True, exist_ok=True)
        args = [str(qemu), "-qtest", f"unix:{sock}", "-qtest-log", "/dev/null",
                "-display", "none", "-audio", "none", "-machine", "q35,cxl=on",
                "-m", "256M", "-nodefaults", "-accel", "qtest",
                "-device", "pxb-cxl,bus_nr=12,bus=pcie.0,id=cxl.0"]
        for node in self.topology:
            tile, root_port, function = node["tile"], node["port"], node["function"]
            if function == 0:
                args += ["-device", f"cxl-rp,port={root_port},bus=cxl.0,id=rp{root_port},chassis=0,slot={root_port + 2},addr={root_port:x}"]
            args += ["-device", f"cxl-type2,bus=rp{root_port},addr=0.{function},multifunction=on,id=t2_{tile},sn={201 + tile},gpu-mode=0,"
                     f"cache-size=16M,mem-size=64M,cxlmemsim-addr=127.0.0.1,cxlmemsim-port={port},"
                     "coherency-enabled=true,latency-enabled=true,read-latency-ns=120,"
                     "write-latency-ns=250,coherency-latency-ns=112,bandwidth-gbps=51,hmc-install-ns=8"
                     + ("," + device_options if device_options else "")]
        (out / "command.json").write_text(json.dumps(args, indent=2) + "\n")
        try:
            self.proc, self.qt, self.log = launch_qemu(args, sock, out / "qemu.log")
            self.qt.sock.settimeout(30)
            self.devices = []
            for node in self.topology:
                tile, bus, base = node["tile"], node["bus"], node["bar2"]
                function = node["function"]
                if function == 0:
                    siblings = sum(n["port"] == node["port"] for n in self.topology)
                    configure_pci_bridge(self.qt, 12, node["port"], 0, 12, bus, bus, base, siblings * 0x08000000)
                identity = pci_read(self.qt, bus, 0, function, 0)
                if identity != 0x0D928086:
                    raise RuntimeError(f"tile {tile}: unexpected PCI identity {identity:#x}")
                for register, address in [(0x18, node["bar2"]), (0x20, node["bar4"])]:
                    pci_write(self.qt, bus, 0, function, register, address & 0xFFFFFFFF)
                    pci_write(self.qt, bus, 0, function, register + 4, address >> 32)
                    actual = (pci_read(self.qt, bus, 0, function, register) & ~15) | (pci_read(self.qt, bus, 0, function, register + 4) << 32)
                    if actual != address:
                        raise RuntimeError(f"tile {tile}: BAR readback {actual:#x} != {address:#x}")
                pci_write(self.qt, bus, 0, function, 4, pci_read(self.qt, bus, 0, function, 4) | 6)
                if self.qt.readl(base) != 0x43584C32:
                    raise RuntimeError(f"tile {tile}: BAR2 not mapped")
                node["pci_identity"] = identity
                node["bar2_magic"] = self.qt.readl(base)
                self.devices.append(Device(self.qt, tile, node["bar2"], node["bar4"]))
            (out / "topology.json").write_text(json.dumps(self.topology, indent=2) + "\n")
        except BaseException:
            self.close()
            raise

    def close(self):
        if self.qt:
            self.qt.close()
        self.stop_process(self.proc)
        if self.log:
            self.log.close()
        self.guard.close()
        self.tmp.cleanup()

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()
