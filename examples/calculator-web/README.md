# Browser calculator shell

This committed browser shell consumes the generated scalar package at
`./generated`. The package can come from the direct single-file calculator or
from the canonical multi-module Project without changing the shell:

```sh
mkdir -p /absolute/direct-calculator /absolute/project-calculator
cp -R examples/calculator-web/. /absolute/direct-calculator/
cp -R examples/calculator-web/. /absolute/project-calculator/

cargo run --locked -p semaprax -- build examples/calculator.spx --target web \
  --export calculator.add \
  --export calculator.subtract \
  --export calculator.multiply \
  --export calculator.divide \
  --export calculator.is-negative \
  --export calculator.not \
  -o /absolute/direct-calculator/generated

cargo run --locked -p semaprax -- build \
  examples/calculator-project/semaprax.toml --target web \
  -o /absolute/project-calculator/generated
```

The interactive UI invokes add, subtract, multiply, and divide by stable ID and
renders normalized semantic failures without losing runtime re-entry. The same
generated declarations expose the complete six-function Project surface.

The locked Chromium fixture can verify both generated packages in one serial
run:

```sh
cd platform-tests/wasm-scalar-browser-v1
npm ci --ignore-scripts
npm run test:fixtures -- /absolute/direct-calculator /absolute/project-calculator
```
