import importlib.util,json,unittest
from pathlib import Path
ROOT=Path(__file__).parent
spec=importlib.util.spec_from_file_location('direct_generator',ROOT/'generate.py')
gen=importlib.util.module_from_spec(spec);spec.loader.exec_module(gen)
class RejectionContract(unittest.TestCase):
 def setUp(self):self.p=json.loads((ROOT/'../compiler/build/projected/program.json').read_bytes())
 def test_unavailable_mutation_entry_rejected(self):
  with self.assertRaises(gen.Unsupported):gen.Generator(self.p,'Item.change').generate()
 def test_unavailable_transaction_entry_rejected(self):
  with self.assertRaises(gen.Unsupported):gen.Generator(self.p,'update_pair').generate()
 def test_reachable_unsupported_expression_rejected(self):
  entry=next(d for d in self.p['declarations'] if d['name']=='probe')
  entry['body'][0]['value']={'op':'notSupported','span':entry['span']}
  with self.assertRaisesRegex(gen.Unsupported,'notSupported'):gen.Generator(self.p,'probe').generate()
 def test_reachable_recursive_call_rejected(self):
  entry=next(d for d in self.p['declarations'] if d['name']=='probe')
  entry['body'][0]['value']={'op':'call','target':'probe','targetId':entry['semanticId'],'arguments':[{'op':'load','path':['input']}]}
  with self.assertRaisesRegex(gen.Unsupported,'recursive'):gen.Generator(self.p,'probe').generate()
if __name__=='__main__':unittest.main()
