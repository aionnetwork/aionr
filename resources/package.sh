#!/bin/bash

## Step 0: touch and force version update
touch util/version/build.rs

## Step 1: build release version
cargo build --release || exit

## Step 2: start packaging
PACKAGE_NAME="aionr-"`sed -n '1p' release`-`date +%Y%m%d`

if [ -z "$1" ] ; then
    echo -e "\033[33mWARN: Using generated name: $PACKAGE_NAME \033[0m"
else
    PACKAGE_NAME=$1
fi

MAINT="package/$PACKAGE_NAME/mainnet/mainnet.toml"
MAINJ="package/$PACKAGE_NAME/mainnet/mainnet.json"
MASTT="package/$PACKAGE_NAME/mastery/mastery.toml"
MASTJ="package/$PACKAGE_NAME/mastery/mastery.json"
CUSTT="package/$PACKAGE_NAME/custom/custom.toml"
CUSTJ="package/$PACKAGE_NAME/custom/custom.json"
AMITYT="package/$PACKAGE_NAME/amity/amity.toml"
AMITYJ="package/$PACKAGE_NAME/amity/amity.json"
LOG_CONFIG="package/$PACKAGE_NAME/log_config.yaml"

## Step 2-1: remove old packages, build template release
rm -rf package/$PACKAGE_NAME

mkdir -p package/$PACKAGE_NAME/mainnet
mkdir package/$PACKAGE_NAME/mastery
mkdir package/$PACKAGE_NAME/custom
mkdir package/$PACKAGE_NAME/amity
mkdir package/$PACKAGE_NAME/libs

## Step 2-2: copy binary and libraries into target directory(package/$1)
cp target/release/aion package/$PACKAGE_NAME
LIBAVMJNI=$(readlink -f target/release/build/avm*/out/libavmloader.so | xargs ls -t | sed -n '1p')
cp $LIBAVMJNI package/$PACKAGE_NAME/libs/libavmloader.so
cp -r vms/avm/libs/aion_vm package/$PACKAGE_NAME/libs

## Step 2-3: generate configuration files
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

<<<<<<< HEAD
## Step 2-4: copy env script
=======
## Step 5: copy env script and log config file
>>>>>>> 5b6df132e4186c588a468b71a2a62928adaee05e
cp resources/env package/$PACKAGE_NAME
cp resources/log_config.yaml package/$PACKAGE_NAME

## Step 2-5: compress
tar -C package -czf ${PACKAGE_NAME}.tar.gz $PACKAGE_NAME
echo "Successfully packaged: $(pwd)/${PACKAGE_NAME}.tar.gz !!!"
