import importlib.util
import pathlib
import unittest
ROOT=pathlib.Path(__file__).parent
SPEC=importlib.util.spec_from_file_location("law16_checked_u32_nonadmission_capsule",ROOT/"law16_checked_u32_nonadmission_capsule.py")
CAPSULE=importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(CAPSULE)
class CheckedU32NonadmissionCapsuleTests(unittest.TestCase):
 def test_retained_pinned_parser_receipt_blocks_u32_substitution(self):
  review=CAPSULE.review(ROOT/"evidence/law16-checked-u32-nonadmission-v1")
  self.assertEqual(review["status"],"authenticated_unsupported_by_pinned_parser")
  self.assertEqual(review["checked_sources"],2)
  self.assertIn("do not substitute i32, i64, u8, or usize",review["consequence"])
if __name__=="__main__": unittest.main()
