#!/bin/sh
# The Vulkan backend needs the SPIRV-Headers CMake package at build time. This unpacks the
# distribution package into vendor/spirv (no root needed); .cargo/config.toml points the build at it.
# Not needed if `spirv-headers` is installed system-wide.
set -eu
cd "$(dirname "$0")/.."
mkdir -p vendor/deb
(cd vendor/deb && apt-get download spirv-headers)
dpkg -x vendor/deb/spirv-headers_*.deb vendor/spirv
echo "SPIRV-Headers unpacked to vendor/spirv"
