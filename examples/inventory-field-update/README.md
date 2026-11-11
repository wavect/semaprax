# Inventory scalar field updates

This compiler-checked source example updates one `count` and one `active` field
inside an owning `Vec<Item>`, retaining the SKU owners and other row values.
Expected result is 42. See the [owning contract](../../docs/OWNED-RECORD-SCALAR-FIELD-UPDATE-V1.md).
The interpreter and native CLI both returned 42 in the focused qualification.
