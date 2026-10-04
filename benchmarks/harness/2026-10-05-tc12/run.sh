#!/bin/bash
export HARNESS_PYTHON=/Users/kevin/.local/bin/python3 SEMAPRAX_HARNESS_HOME=/private/tmp/claude-501/tc/p12/home PATH=/usr/bin:/bin:/Users/kevin/.local/bin:$PATH
cd /private/tmp/claude-501/tc/p12/project
exec /private/tmp/claude-501/tc/e/target/private/debug/semaprax-harness bench app run /private/tmp/claude-501/tc/e/crates/semaprax-harness/tests/fixtures/bench/apptasks --out $OUT --work /private/tmp/claude-501/tc/p12/work --reps $REPS --profile-arms ${ARMS:-defaults,tiers,feedback-allowance,prompt-renderer,combined} \
 --model id=claude-haiku-4-5,size=large,billed=1 --production-adapter org.wavect/haiku-cli-shim --production-project /private/tmp/claude-501/tc/p12/project \
 --max-usd 5 ${EXTRA} \
 --env HARNESS_PYTHON=/Users/kevin/.local/bin/python3 --env HARNESS_NODE=/Users/kevin/.nvm/versions/node/v24.3.0/bin/node \
 --env HARNESS_TIKTOKEN_PYTHON=/private/tmp/claude-501/hp-tools/tiktoken-venv/bin/python --env HARNESS_TIKTOKEN_CACHE=/private/tmp/claude-501/hp-tools/tiktoken-cache
