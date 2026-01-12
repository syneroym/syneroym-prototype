#!/bin/bash
set -e

# Check if cargo-llvm-cov is installed
if ! command -v cargo-llvm-cov &> /dev/null; then
    echo "cargo-llvm-cov could not be found. Installing..."
    cargo install cargo-llvm-cov
fi

echo "Running coverage..."
# Run coverage for the workspace
# --html: generate HTML report
# --open: open the report
cargo llvm-cov --workspace --html --output-dir coverage

echo "Coverage report generated at coverage/"