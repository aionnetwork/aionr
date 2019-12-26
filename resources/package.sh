#!/bin/bash


PACKAGE_NAME="oanr-v"`sed -n '1p' release`-`date +%Y%m%d`

if [ -z "$1" ] ; then
    echo -e "\033[33mWARN: Using generated name: $PACKAGE_NAME \033[0m"
else
    PACKAGE_NAME=$1
fi

rm -rf target/release/build/aion-version* target/release/build/avm-* || echo "cannot find previous avm and version build"

MAINT="package/$PACKAGE_NAME/mainnet/mainnet.toml"
MAINJ="package/$PACKAGE_NAME/mainnet/mainnet.json"
MASTT="package/$PACKAGE_NAME/mastery/mastery.toml"
MASTJ="package/$PACKAGE_NAME/mastery/mastery.json"
CUSTT="package/$PACKAGE_NAME/custom/custom.toml"
CUSTJ="package/$PACKAGE_NAME/custom/custom.json"
AMITYT="package/$PACKAGE_NAME/amity/amity.toml"
AMITYJ="package/$PACKAGE_NAME/amity/amity.json"
LOG_CONFIG="package/$PACKAGE_NAME/log_config.yaml"

## Step 0: remove old packages, build template release
rm -rf package/$PACKAGE_NAME

mkdir -p package/$PACKAGE_NAME/mainnet
mkdir package/$PACKAGE_NAME/mastery
mkdir package/$PACKAGE_NAME/custom
mkdir package/$PACKAGE_NAME/amity
mkdir package/$PACKAGE_NAME/libs

## Step 1: build release version
cargo build --release

## Step 2: copy binary and libraries into target directory(package/$1)
cp target/release/aion package/$PACKAGE_NAME
LIBAVMJNI=$(readlink -f target/release/build/avm*/out/libavmloader.so | xargs ls -t | sed -n '1p')
cp $LIBAVMJNI package/$PACKAGE_NAME/libs/libavmloader.so
cp -r vms/avm/libs/aion_vm package/$PACKAGE_NAME/libs

## Step 3: generate configuration files
cp resources/config_mainnet.toml $MAINT
cp resources/mainnet.json $MAINJ
echo -e '#!/bin/bash \n./env\nsource custom.env\nexport AIONR_HOME=.\nexport LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$AIONR_HOME/libs\n./aion --config=mainnet/mainnet.toml --log-config=log_config.yaml $*'>package/$PACKAGE_NAME/mainnet.sh
chmod +x package/$PACKAGE_NAME/mainnet.sh

cp resources/config_mastery.toml $MASTT
cp resources/mastery.json $MASTJ
echo -e '#!/bin/bash \n./env\nsource custom.env\nexport AIONR_HOME=.\nexport LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$AIONR_HOME/libs\n./aion --config=mastery/mastery.toml $*'>package/$PACKAGE_NAME/mastery.sh
chmod +x package/$PACKAGE_NAME/mastery.sh

cp resources/config_custom.toml $CUSTT
cp resources/custom.json $CUSTJ
echo -e '#!/bin/bash \n./env\nsource custom.env\nexport AIONR_HOME=.\nexport LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$AIONR_HOME/libs\n./aion --config=custom/custom.toml $*'>package/$PACKAGE_NAME/custom.sh
chmod +x package/$PACKAGE_NAME/custom.sh

cp resources/config_amity.toml $AMITYT
cp resources/amity.json $AMITYJ
echo -e '#!/bin/bash \n./env\nsource custom.env\nexport AIONR_HOME=.\nexport LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$AIONR_HOME/libs\n./aion --config=amity/amity.toml $*'>package/$PACKAGE_NAME/amity.sh
chmod +x package/$PACKAGE_NAME/amity.sh

## Step 5: copy env script and log config file
cp resources/env package/$PACKAGE_NAME
cp resources/log_config.yaml package/$PACKAGE_NAME

## Step 6: compress
tar -C package -czf ${PACKAGE_NAME}.tar.gz $PACKAGE_NAME
echo "Successfully packaged: $(pwd)/${PACKAGE_NAME}.tar.gz !!!"
