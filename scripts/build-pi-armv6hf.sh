#!/usr/bin/env bash

# Snipped from https://github.com/librespot-org/librespot/blob/dev/contrib/docker-build-pi-armv6hf.sh and edited for libusb
# Originally snipped and tucked from https://github.com/plietar/librespot/pull/202/commits/21549641d39399cbaec0bc92b36c9951d1b87b90
# and further inputs from https://github.com/kingosticks/librespot/commit/c55dd20bd6c7e44dd75ff33185cf50b2d3bd79c3

set -eux

# Collect Paths
SYSROOT="/pi-tools/arm-bcm2708/arm-bcm2708hardfp-linux-gnueabi/arm-bcm2708hardfp-linux-gnueabi/sysroot"
TOOLCHAIN="/pi-tools/arm-bcm2708/gcc-linaro-arm-linux-gnueabihf-raspbian-x64/"
GCC="$TOOLCHAIN/bin"
GCC_SYSROOT="$GCC/gcc-sysroot"


# Download dependencies to a tmp dir
if [ ! -d /tmp/debs ]; then
  mkdir -p /tmp/deb-download
  pushd /tmp/deb-download
  mkdir /tmp/debs
  git clone https://github.com/Ragnaroek/rust-on-raspberry-docker.git
  cd rust-on-raspberry-docker/apt
  ./install-keys.sh
  ./download.sh libhidapi-libusb0 libhidapi-dev libusb-1.0-0-dev libc6-dev libssl-dev libudev-dev
  mv *.deb /tmp/debs
  popd
fi

export PATH=$TOOLCHAIN/bin/:$PATH
export PKG_CONFIG_LIBDIR=${SYSROOT}/usr/lib/arm-linux-gnueabihf/pkgconfig/
export PKG_CONFIG_SYSROOT_DIR=$SYSROOT
export PKG_CONFIG_ALL_STATIC=on

# Link the compiler
export TARGET_CC="$GCC/arm-linux-gnueabihf-gcc"

# Create wrapper around gcc to point to rpi sysroot
echo -e '#!/bin/bash' "\n$TARGET_CC --sysroot $SYSROOT \"\$@\"" > $GCC_SYSROOT
chmod +x $GCC_SYSROOT

if [ ! -f /tmp/sysroot-dl ]; then
  # Add extra target dependencies to our rpi sysroot
  for path in /tmp/debs/*; do
    dpkg -x $path $SYSROOT
  done
  touch /tmp/sysroot-dl
fi

mkdir -p ~/.cargo/

# point cargo to use gcc wrapper as linker
echo -e '[target.arm-unknown-linux-gnueabihf]\nlinker = "gcc-sysroot"\nstrip = { path = "arm-linux-gnueabihf-strip" }\nobjcopy = { path = "arm-linux-gnueabihf-objcopy" }' > ~/.cargo/config.toml

# Somehow .cargo/config.toml's linker settings are ignored
export RUSTFLAGS="-C linker=gcc-sysroot"
export CC_ARM_UNKNOWN_LINUX_GNUEABIHF=gcc-sysroot
export CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_LINKER="gcc-sysroot -ldl"

# fix hidapi build issue
export CFLAGS="-std=c99"

# Overwrite libc and libpthread with the new ones since the sysroot ones are outdated
cp $SYSROOT/lib/arm-linux-gnueabihf/libc-2.28.so $SYSROOT/lib/libc.so.6
cp $SYSROOT/lib/arm-linux-gnueabihf/libdl-2.28.so $SYSROOT/lib/libdl.so.2
cp $SYSROOT/lib/arm-linux-gnueabihf/libpthread-2.28.so $SYSROOT/lib/libpthread.so.0

# Remove conflicting static libraries
rm -f $SYSROOT/usr/lib/arm-linux-gnueabihf/libdl.a
rm -f $SYSROOT/usr/lib/arm-linux-gnueabihf/libpthread.a

CMD=$1
shift

# Build
cargo $CMD --target arm-unknown-linux-gnueabihf "$@"

