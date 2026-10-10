# Typed record ordering in Project v31

`src/data.spx` stores rows as `Vec<Row>` inside a `Report` with nested `Metrics`.
The full fixture fills its four-slot vector and ties the first two sort keys;
`vec_sort_owned<Row>` compares fields in declaration order, so department,
priority, then the UTF-8 bytes of `id` determine the permutation. Consuming
`for own` traversal folds the sorted ordinal fields into `4321`. The empty
fixture exercises sort and traversal with no rows.

Check and run the project with the selected native command route:

```sh
semaprax check .
semaprax test .
semaprax build --manifest-path semaprax.toml --target native --output app
printf 'run\n' | ./app
```

The command prints `4321` for nonempty input after both ordering witnesses pass.
Project v31 nested collection-record execution qualification remains pending;
the owning project regression is `v31_public_typed_record_order_example`.
