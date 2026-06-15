#!/usr/bin/env bash
set -euo pipefail

cargo bench -p oxidepdf-core --bench workflow_hot_paths -- --test
cargo bench -p oxidepdf-cli --bench cli_workflow_paths -- --test
