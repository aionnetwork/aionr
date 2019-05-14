#!/bin/bash

if [ ! -n "$1" ] ; then
    echo "Error: You need to give a package name"
    exit 1
fi

MAINT="package/$1/mainnet/mainnet.toml"
MAINJ="package/$1/mainnet/mainnet.json"
MASTT="package/$1/mastery/mastery.toml"
MASTJ="package/$1/mastery/mastery.json"
CUSTT="package/$1/custom/custom.toml"
CUSTJ="package/$1/custom/custom.json"
#AVMTT="package/$1/avmtestnet/avmtestnet.toml"
#AVMTJ="package/$1/avmtestnet/avmtestnet.json"

mkdir -p package/$1/mainnet
mkdir  package/$1/mastery
mkdir  package/$1/custom
#mkdir  package/$1/avmtestnet
mkdir  package/$1/libs

cargo build --release

cp target/release/aion package/$1
LIBAVMJNI=$(readlink -f target/release/build/avm*/out/libavmjni.so)
cp $LIBAVMJNI package/$1/libs/libavmjni.so
cp -r vms/avm/libs/aion_vm package/$1/libs


cp aion/cli/config_mainnet.toml $MAINT
cp core/res/aion/mainnet.json $MAINJ
sed -i '1i [aion]' $MAINT
sed -i '2i chain = \"mainnet/mainnet.json\"' $MAINT
echo -e 'export AIONR_HOME=.\nexport LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$AIONR_HOME/libs\n./aion --config=mainnet/mainnet.toml $*'>package/$1/mainnet.sh
chmod +x package/$1/mainnet.sh

cp aion/cli/config_mastery.toml $MASTT
cp core/res/aion/testnet_mastery.json $MASTJ
sed -i '/\<chain = /c chain = \"mastery/mastery.json\"' $MASTT
echo -e 'export AIONR_HOME=.\nexport LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$AIONR_HOME/libs\n./aion --config=mastery/mastery.toml $*'>package/$1/mastery.sh
chmod +x package/$1/mastery.sh

cp aion/cli/config_custom.toml $CUSTT
cp core/res/aion/custom.json $CUSTJ
sed -i '/\<chain = /c chain = \"custom/custom.json\"' $CUSTT
echo -e 'export AIONR_HOME=.\nexport LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$AIONR_HOME/libs\n./aion --config=custom/custom.toml $*'>package/$1/custom.sh
chmod +x package/$1/custom.sh

#cp aion/cli/avmtestnet.toml $AVMTT
#cp core/res/aion/avmtestnet.json $AVMTJ
#sed -i '/\<chain = /c chain = \"avmtestnet/avmtestnet.json\"' $AVMTT
#echo -e 'export AIONR_HOME=.\nexport LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$AIONR_HOME/libs \n./aion --config=avmtestnet/avmtestnet.toml $*'>package/$1/avmtestnet.sh
#chmod +x package/$1/avmtestnet.sh

tar -C package -czf ${1}.tar.gz $1
echo "Successfully packaged: $(pwd)/${1}.tar.gz !!!"
