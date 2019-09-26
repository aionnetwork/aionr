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

## Step 0: remove old packages, build template release
rm -rf package/$1

mkdir -p package/$1/mainnet
mkdir package/$1/mastery
mkdir package/$1/custom
mkdir package/$1/amity
mkdir package/$1/libs

## Step 1: build release version
cargo build --release

## Step 2: copy binary and libraries into target directory(package/$1)
cp target/release/aion package/$1
LIBAVMJNI=$(readlink -f target/release/build/avm*/out/libavmjni.so | xargs ls -t | sed -n '1p')
cp $LIBAVMJNI package/$1/libs/libavmjni.so
cp -r vms/avm/libs/aion_vm package/$1/libs

## Step 3: generate configuration files
cp resources/config_mainnet.toml $MAINT
cp resources/mainnet.json $MAINJ
echo -e '#!/bin/bash \n./env\nsource custom.env\nexport AIONR_HOME=.\nexport LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$AIONR_HOME/libs\n./aion --config=mainnet/mainnet.toml $*'>package/$1/mainnet.sh
chmod +x package/$1/mainnet.sh

cp resources/config_mastery.toml $MASTT
cp resources/mastery.json $MASTJ
echo -e '#!/bin/bash \n./env\nsource custom.env\nexport AIONR_HOME=.\nexport LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$AIONR_HOME/libs\n./aion --config=mastery/mastery.toml $*'>package/$1/mastery.sh
chmod +x package/$1/mastery.sh

cp resources/config_custom.toml $CUSTT
cp resources/custom.json $CUSTJ
echo -e '#!/bin/bash \n./env\nsource custom.env\nexport AIONR_HOME=.\nexport LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$AIONR_HOME/libs\n./aion --config=custom/custom.toml $*'>package/$1/custom.sh
chmod +x package/$1/custom.sh

cp resources/config_amity.toml $AMITYT
cp resources/amity.json $AMITYJ
echo -e '#!/bin/bash \n./env\nsource custom.env\nexport AIONR_HOME=.\nexport LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$AIONR_HOME/libs\n./aion --config=amity/amity.toml $*'>package/$1/amity.sh
chmod +x package/$1/amity.sh

## Step 5: copy env script
cp resources/env package/$1

## Step 6: compress
tar -C package -czf ${1}.tar.gz $1
echo "Successfully packaged: $(pwd)/${1}.tar.gz !!!"
