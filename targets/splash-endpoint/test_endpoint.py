import copy
import unittest
from driver import schedule_oracle, oracle
from experiment import matrix


class EndpointOracleTests(unittest.TestCase):
    def test_shared_link_serializes_but_propagation_overlaps(self):
        c=dict(record_ns=64,enforce=True,compute_ns=1,bandwidth=8)
        r=schedule_oracle([dict(tile=t,words=8,release=0) for t in range(2)],c)
        self.assertEqual(r[0]['compute_start'],r[1]['compute_start'])
        self.assertEqual(r[1]['queue_wait'],8)
        self.assertEqual(r[1]['finished']-r[0]['finished'],8)
        self.assertLess(r[1]['link_start'],r[0]['link_end'])

    def test_earlier_compute_completion_wins_link(self):
        c=dict(record_ns=0,enforce=False,compute_ns=1,bandwidth=8)
        r=schedule_oracle([dict(tile=0,words=128,release=0),dict(tile=1,words=8,release=0)],c)
        self.assertLess(r[1]['link_start'],r[0]['link_start'])

    def test_uint64_wraparound(self):
        self.assertEqual(oracle(bytes.fromhex('ffffffffffffffff'),2),bytes.fromhex('ffffffffffffffff'))

    def test_matrix_has_controls_and_no_duplicates(self):
        import json
        cells=matrix()
        self.assertEqual(len(cells),336)
        self.assertEqual(len(set(json.dumps(c,sort_keys=True) for c in cells)),336)
        self.assertEqual(sum(c['kind']=='scaling' for c in cells),288)


if __name__=='__main__':unittest.main()
