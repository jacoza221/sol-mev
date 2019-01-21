#!/usr/bin/env bash
set -e

testCmd="$*"
genPipeline=false

cd "$(dirname "$0")/.."

# Clear cached json keypair files
rm -rf "$HOME/.config/solana"

source ci/_
export RUST_BACKTRACE=1
export RUSTFLAGS="-D warnings"
export PATH=$PWD/target/debug:$PATH
export USE_INSTALL=1

if [[ -n $BUILDKITE && -z $testCmd ]]; then
  genPipeline=true
  echo "
steps:
  "
fi

build() {
  $genPipeline && return
  ci/version-check-with-upgrade.sh stable
  _ scripts/ulimit-n.sh
  FEATURES=""
  _ cargo build --all --features="$FEATURES"
}

runTest() {
  declare runTestName="$1"
  declare runTestCmd="$2"
  if $genPipeline; then
    echo "
  - command: \"$0 '$runTestCmd'\"
    name: \"$runTestName\"
    timeout_in_minutes: 30
"
    return
  fi

  if [[ -n $testCmd && "$testCmd" != "$runTestCmd" ]]; then
    echo Skipped "$runTestName"...
    return
  fi
  #shellcheck disable=SC2068 # Don't want to double quote $runTestCmd
  $runTestCmd
}

build

runTest "Leader rotation off" \
  "ci/localnet-sanity.sh -b -i 128"

runTest "Leader rotation off, periodic node restart" \
  "ci/localnet-sanity.sh -b -i 128 -k 16"

runTest "Leader rotation on" \
  "ci/localnet-sanity.sh -i 128"

runTest "Leader rotation on, periodic node restart" \
  "ci/localnet-sanity.sh -i 128 -k 16"

