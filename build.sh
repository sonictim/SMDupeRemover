#!/bin/sh

# Set the binary name
BINARY_NAME="SMDupeRemover"

# Add the necessary targets
rustup target add aarch64-apple-darwin x86_64-apple-darwin

# Build for both architectures
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

# Create the universal binary
lipo -create -output $BINARY_NAME target/aarch64-apple-darwin/release/$BINARY_NAME target/x86_64-apple-darwin/release/$BINARY_NAME

# Verify the binary
file $BINARY_NAME