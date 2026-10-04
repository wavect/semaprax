import importlib.util
import pathlib
import unittest
ROOT=pathlib.Path(__file__).parent
SPEC=importlib.util.spec_from_file_location("law16_checked_u32_nonadmission",ROOT/"law16_checked_u32_nonadmission.py")
NONADMISSION=importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(NONADMISSION)
class CheckedU32NonadmissionTests(unittest.TestCase):
 def test_pinned_commit_and_explicit_non_substitution_consequence(self):
  self.assertEqual(NONADMISSION.PINNED_SEMAPRAX_COMMIT,"9a9db7a8117ac8d292b24ffd5671ec3333272290")
  self.assertIn("do not substitute i32, i64, u8, or usize",NONADMISSION.main.__doc__ or "do not substitute i32, i64, u8, or usize")
 def test_parser_nonadmission_requires_exact_diagnostic_and_nonzero_exit(self):
  root=pathlib.Path(self.enterContext(__import__('tempfile').TemporaryDirectory()))
  (root/'stdout').write_text('{"code":"SPX-P003","message":"integer literals accept only an `i32`, `u8`, or `usize` suffix"}')
  row={"exit_code":1,"timed_out":False,"stdout":{"path":"stdout"},"stderr":{"path":"missing"}}
  self.assertTrue(NONADMISSION.expected_parser_nonadmission(row,root))
  row["exit_code"]=0
  self.assertFalse(NONADMISSION.expected_parser_nonadmission(row,root))
if __name__=="__main__": unittest.main()
