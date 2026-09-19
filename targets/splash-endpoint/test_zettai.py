import unittest
from zettai import plan_zettai
from scale256 import matrix
from transport import plan_topology

class TopologyTests(unittest.TestCase):
    def test_256_functions_have_unique_resources_and_fit_one_domain(self):
        nodes=plan_zettai(256)
        self.assertEqual(len({(n['bus'],n['function']) for n in nodes}),256)
        self.assertEqual(len({n['serial'] for n in nodes}),256)
        self.assertEqual(len({n['switch'] for n in nodes}),4)
        self.assertEqual(len({n['dsp_id'] for n in nodes}),32)
        self.assertLess(max(n['bus'] for n in nodes),256)
        intervals=sorted((n[k],n[k]+size) for n in nodes for k,size in [('bar2',16<<20),('bar4',64<<20)])
        self.assertTrue(all(a[1]<=b[0] for a,b in zip(intervals,intervals[1:])))
        self.assertGreater(intervals[0][0],1<<32)

    def test_bridge_boundaries_and_partial_slots(self):
        for n in [1,7,8,9,63,64,65,255,256]:
            ns=plan_zettai(n)
            self.assertEqual(len(ns),n)
            self.assertEqual(ns[-1]['tile'],n-1)
            self.assertEqual(len([x for x in ns if x['function']==0]),(n+7)//8)

    def test_invalid_counts_rejected_before_launch(self):
        for n in [0,257,-1,True,1.5]:
            with self.assertRaises(ValueError):plan_zettai(n)
            with self.assertRaises(ValueError):plan_topology(n)

    def test_complete_matrix_includes_256_controls(self):
        cells=matrix();self.assertEqual(len(cells),693)
        self.assertEqual(sum(c['n']==256 and c['kind']=='scaling' for c in cells),72)
        self.assertEqual({c['n'] for c in cells},set(2**i for i in range(9)))

if __name__=='__main__':unittest.main()
