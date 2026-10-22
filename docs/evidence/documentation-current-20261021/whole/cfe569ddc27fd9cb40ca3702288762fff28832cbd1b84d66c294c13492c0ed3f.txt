# Independent old default-teardown test receiver

Source-only one-call repair on d719ea241120655b618ca85a185d5fe5c575a2bf. The shared typed_cost.rs tests are instantiated under both borrowed_imports::typed::typed_cost and borrowed_imports::typed::before_alias::typed_cost. The old super::super::super lookup reaches typed and before_alias respectively; both import the new three-argument typed teardown helper. Adding a Receiver there would route the old comparison through the new algorithm and defeat the existing new > old assertion.

The explicit absolute path instead reaches borrowed_imports.rs::pay_default_teardown, the complete original two-argument helper, in both instantiations. Its private visibility covers these descendants. All original assertions, payment formulas, producer functions and fixture data remain byte-exact. No new Receiver, credit, refund or assertion change. Complete immediate beforeimage and unique one-edit inverse retained. No Cargo, compilation, tests or Clippy executed.

Successor composition must project this changed full typed_cost source to its d719 predecessor before immutable cost825 and other historical source-reader tables. Do not rewrite their expected hashes or runtime JSON. No new test body; the existing typed_constructor_census_keeps_all_original_raw_fields_order_and_teardown_formulas owner in both instantiations is affected.
