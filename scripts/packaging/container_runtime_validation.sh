#!/usr/bin/env bash

set -euo pipefail
exec cargo run --quiet --bin gewyvern_validate -- container-runtime-validation "$@"
