#!/bin/bash
DEST=build/rmstream-rmpp
rm -rf $DEST
mkdir $DEST/backend -p
cp icon.png manifest.json $DEST/
rcc --binary -o $DEST/resources.rcc application.qrc
cd backend
cargo build --release --target aarch64-unknown-linux-gnu
cd ..
cp -rv backend/target/aarch64-unknown-linux-gnu/release/stream2 $DEST/backend/entry
