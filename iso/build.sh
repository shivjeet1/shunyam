#!/bin/sh
set -e

echo "Building Shunya Alpine ISO..."

# 1. Clone Alpine aports if not present
if [ ! -d "aports" ]; then
    echo "Cloning alpine aports..."
    git clone --depth 1 https://gitlab.alpinelinux.org/alpine/aports.git
fi

# 2. Copy our profile into aports/scripts
cp mkimg.shunya.sh aports/scripts/

# 3. Create the overlay tarball (apkovl)
echo "Building overlay..."
cd overlay
tar czf ../shunya.apkovl.tar.gz .
cd ..

# 4. Run mkimage.sh (requires Alpine environment or Docker)
echo "Running mkimage.sh (requires abuild environment)..."
echo "To build, run the following in an Alpine container or system:"
echo "  apk add alpine-sdk build-base apk-tools alpine-conf squashfs-tools xorriso mtools dosfstools grub-efi"
echo "  cd aports/scripts"
echo "  sh mkimage.sh --tag edge \\"
echo "    --outdir /tmp \\"
echo "    --arch x86_64 \\"
echo "    --repository http://dl-cdn.alpinelinux.org/alpine/edge/main \\"
echo "    --repository http://dl-cdn.alpinelinux.org/alpine/edge/community \\"
echo "    --profile shunya"

echo ""
echo "Note: The apkovl should be placed in the build environment to be embedded or passed as an argument."
