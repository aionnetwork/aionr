#!/bin/bash

if [ ! -n "$1" ] ; then
    echo "Error: You need to give a package name"
    exit 1
fi

# rm -rf target/release/build/aion-version* target/release/build/avm-* || echo "cannot find previous avm and version build"

MAINT="package/$1/mainnet/mainnet.toml"
MAINJ="package/$1/mainnet/mainnet.json"
MASTT="package/$1/mastery/mastery.toml"
MASTJ="package/$1/mastery/mastery.json"
CUSTT="package/$1/custom/custom.toml"
CUSTJ="package/$1/custom/custom.json"
AMITYT="package/$1/amity/amity.toml"
AMITYJ="package/$1/amity/amity.json"

mkdir -p package/$1/mainnet
mkdir  package/$1/mastery
mkdir  package/$1/custom
mkdir  package/$1/amity
mkdir  package/$1/libs

cargo build --release

cp target/release/aion package/$1
LIBAVMJNI=$(readlink -f target/release/build/avm*/out/libavmjni.so)
cp $LIBAVMJNI package/$1/libs/libavmjni.so
cp -r vms/avm/libs/aion_vm package/$1/libs


cp resources/config_mainnet.toml $MAINT
cp resources/mainnet.json $MAINJ
echo -e '#!/usr/bin/env sh\nexport AIONR_HOME=.\nexport LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$AIONR_HOME/libs\n./aion --config=mainnet/mainnet.toml $*'>package/$1/mainnet.sh
chmod +x package/$1/mainnet.sh

cp resources/config_mastery.toml $MASTT
cp resources/mastery.json $MASTJ
echo -e '#!/usr/bin/env sh\nexport AIONR_HOME=.\nexport LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$AIONR_HOME/libs\n./aion --config=mastery/mastery.toml $*'>package/$1/mastery.sh
chmod +x package/$1/mastery.sh

cp resources/config_custom.toml $CUSTT
cp resources/custom.json $CUSTJ
echo -e '#!/usr/bin/env sh\nexport AIONR_HOME=.\nexport LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$AIONR_HOME/libs\n./aion --config=custom/custom.toml $*'>package/$1/custom.sh
chmod +x package/$1/custom.sh

cp resources/config_amity.toml $AMITYT
cp resources/amity.json $AMITYJ
echo -e '#!/usr/bin/env sh\nexport AIONR_HOME=.\nexport LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$AIONR_HOME/libs\n./aion --config=amity/amity.toml $*'>package/$1/amity.sh
chmod +x package/$1/amity.sh

tar -C package -czf ${1}.tar.gz $1
echo "Successfully packaged: $(pwd)/${1}.tar.gz !!!"
