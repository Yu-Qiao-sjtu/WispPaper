#!/bin/sh
# What a Debian or Ubuntu system needs to build the app: GPUI draws through
# Vulkan and talks to Wayland and X11, and finds fonts through fontconfig.
set -eu
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
    build-essential pkg-config clang cmake \
    libfontconfig-dev libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev \
    libx11-dev libx11-xcb-dev libxcb1-dev libvulkan-dev libssl-dev libzstd-dev
