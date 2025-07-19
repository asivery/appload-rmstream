#!/bin/bash
DEST=build/rmstream-rm2
rm -rf $DEST
mkdir -p $DEST/backend
cp icon.png manifest.json $DEST/
/usr/lib/qt6/libexec/rcc --binary -o $DEST/resources.rcc application.qrc
cd backend
cargo build --release --target armv7-unknown-linux-gnueabihf
cd ..
cp -rv backend/target/armv7-unknown-linux-gnueabihf/release/stream2 $DEST/backend/entry
