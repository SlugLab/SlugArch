import hashlib
import unittest

from experiment import Barrier, BarrierError, certificate, check_certificate, corrupt


class MemoryDevice:
    def __init__(self):
        self.memory = {}

    def device_write(self, offset, data):
        self.memory[offset] = data

    def consume(self, offset, size):
        value = self.memory[offset]
        assert len(value) == size
        return value


class MemorySimulator:
    def __init__(self):
        self.devices = [MemoryDevice() for _ in range(8)]


class BarrierTests(unittest.TestCase):
    def test_record_loss_blocks_release_despite_present_output_hash(self):
        for mode in ("event", "tile", "nested2", "nested4"):
            events = []
            hashes = {t: [hashlib.sha256(bytes([t, b])).digest() for b in range(16)] for t in range(8)}
            b = Barrier(MemorySimulator(), 1, mode, hashes, events, "missing_record")
            with self.assertRaises(BarrierError) as error:
                b.execute()
            self.assertEqual(error.exception.detail["tile"], 7)
            self.assertEqual(error.exception.detail["fault"], "missing_record")
            self.assertFalse(b.metrics["released"])
            self.assertFalse(any(e["action"] == "release" for e in events))

    def test_exact_membership_required_even_with_same_payload_hashes(self):
        hashes = [hashlib.sha256(b"same output").digest()]
        expected = certificate(3, 2, 0, [0, 1], hashes, 2)
        wrong = certificate(3, 2, 0, [0, 2], hashes, 2)
        with self.assertRaises(BarrierError) as error:
            check_certificate(wrong, expected, 0, 3, "root")
        self.assertEqual(error.exception.detail["fault"], "wrong_membership")

    def test_stale_and_corrupt_receipts_cannot_be_reused(self):
        cert = certificate(2, 1, 3, [3], [hashlib.sha256(b"v").digest()], 1)
        for kind in ("stale_epoch", "wrong_tile", "corrupt_digest"):
            with self.assertRaises(BarrierError) as error:
                check_certificate(corrupt(cert, kind), cert, 3, 2, "root")
            self.assertEqual(error.exception.detail["fault"], kind)


if __name__ == "__main__":
    unittest.main()
