#!/usr/bin/env bash

docker run --rm -v .:/work --device /dev/fuse --cap-add SYS_ADMIN -i ubuntu:24.04 bash <<'EOF'

set -ex

apt-get update
DEBIAN_FRONTEND=noninteractive apt-get install -y pkg-config libgtk-4-dev libadwaita-1-dev curl wget build-essential file

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable

cd /work
/root/.cargo/bin/cargo build --release

wget https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
chmod +x linuxdeploy-x86_64.AppImage

mkdir -p AppDir/usr/bin
cp target/release/redirector AppDir/usr/bin/

./linuxdeploy-x86_64.AppImage \
  --appdir AppDir \
  -e AppDir/usr/bin/redirector \
  -d data/redirector.desktop \
  -i data/redirector.png \
  --output appimage

rm -r linuxdeploy-x86_64.AppImage AppDir

EOF
