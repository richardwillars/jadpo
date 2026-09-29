"""Checks that the validation gate cannot silently lose evidence."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

REPOSITORY = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('jadpo_verify', REPOSITORY / 'tools/verify.py')
verify = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verify)


class EvidenceInventoryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)
        self.addCleanup(patch.stopall)
        patch.object(verify, 'ROOT', self.root).start()
        self.write('tests/runtime/example.test.ts', '// test source')
        self.write('examples/sample/app.jadpo', 'type Name = Text {}')
        self.write('examples/golden-todo/app.jadpo', '// design contract')
        acceptance = json.dumps({'cases': [{'id': 'AUTH-001'}]})
        self.write('examples/golden-todo/acceptance.json', acceptance)
        self.obligations = {'acceptance_sha256': hashlib.sha256(acceptance.encode()).hexdigest(),
                            'cases': {'AUTH-001': {'status': 'not_executed', 'gap': 'No integrated runtime evidence'}}}
        self.write_obligations()
        self.manifest = {'runtime_suites': ['tests/runtime/example.test.ts'], 'postgres_suites': {},
                         'examples': {'examples/sample': {'status': 'runnable', 'reason': 'Example'},
                                      'examples/golden-todo': {'status': 'design_contract', 'reason': 'Not executable'}}}

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value)

    def write_obligations(self):
        self.write('tests/validation/golden-obligations.json', json.dumps(self.obligations))

    def test_registered_evidence_is_accepted_without_claiming_golden_execution(self):
        result = verify.inventory(self.manifest)
        self.assertEqual(result['cases']['AUTH-001']['status'], 'not_executed')

    def test_new_nested_runtime_suite_cannot_be_silently_omitted(self):
        self.write('tests/runtime/auth/revocation.test.ts', '// newly authored suite')
        with self.assertRaisesRegex(ValueError, 'inventory differs'):
            verify.inventory(self.manifest)

    def test_duplicate_suite_does_not_count_as_extra_evidence(self):
        self.manifest['runtime_suites'] *= 2
        with self.assertRaisesRegex(ValueError, 'inventory differs'):
            verify.inventory(self.manifest)

    def test_deleted_suite_is_not_a_pass(self):
        (self.root/'tests/runtime/example.test.ts').unlink()
        with self.assertRaisesRegex(ValueError, 'inventory differs'):
            verify.inventory(self.manifest)

    def test_example_with_only_entity_dossiers_still_requires_classification(self):
        self.write('examples/new-app/entities/customer.jadpo', 'entity Customer {}')
        with self.assertRaisesRegex(ValueError, 'explicitly classified'):
            verify.inventory(self.manifest)

    def test_removing_a_golden_obligation_is_rejected(self):
        self.obligations['cases'] = {}
        self.write_obligations()
        with self.assertRaisesRegex(ValueError, 'Every golden acceptance'):
            verify.inventory(self.manifest)

    def test_changing_acceptance_requires_revisiting_the_evidence_map(self):
        self.write('examples/golden-todo/acceptance.json', json.dumps({'cases': [{'id': 'AUTH-001', 'expect': {'status': 200}}]}))
        with self.assertRaisesRegex(ValueError, 'Golden acceptance changed'):
            verify.inventory(self.manifest)

    def test_a_claimed_pass_without_an_executable_case_is_rejected(self):
        self.obligations['cases']['AUTH-001']['status'] = 'passed'
        self.write_obligations()
        with self.assertRaisesRegex(ValueError, 'unexecuted obligation'):
            verify.inventory(self.manifest)

    def test_quarantine_requires_an_explanation(self):
        self.manifest['examples']['examples/sample'] = {'status': 'legacy', 'reason': ''}
        with self.assertRaisesRegex(ValueError, 'Invalid example classification'):
            verify.inventory(self.manifest)


if __name__ == '__main__':
    unittest.main()
