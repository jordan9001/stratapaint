#!/bin/bash

set -e
set -v

# this should eventually be a Makefile, but I am using a .sh file for now
# also this does all debug stuff for now

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

rm -rf ${DIR}/build
mkdir -p ${DIR}/build

# copy over the site
cp -r ${DIR}/site ${DIR}/build/site

# make the wasm
wasm-pack build --dev --no-typescript --target web ${DIR}/clientwasm/

# copy over the wasm
rm ${DIR}/clientwasm/pkg/package.json
cp ${DIR}/clientwasm/pkg/* ${DIR}/build/site/

# make the server
pushd ${DIR}/server/
cargo build
popd

# copy over the server
cp ${DIR}/server/target/debug/gameserver ${DIR}/build/

# done!

pushd ${DIR}/build
./gameserver

popd
