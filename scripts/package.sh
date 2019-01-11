#!/bin/bash
MAINT="package/$1/mainnet/mainnet.toml"
MAINJ="package/$1/mainnet/mainnet.json"
MASTT="package/$1/mastery/mastery.toml"
MASTJ="package/$1/mastery/mastery.json"
CUSTT="package/$1/custom/custom.toml"
CUSTJ="package/$1/custom/custom.json"

mkdir -p package/$1/mainnet
mkdir  package/$1/mastery
mkdir  package/$1/custom

cargo build --release

cp target/release/aion package/$1

cp aion/cli/config_mainnet.toml $MAINT
cp core/res/aion/mainnet.json $MAINJ
sed -i '1i [aion]' $MAINT
sed -i '2i chain = \"mainnet/mainnet.json\"' $MAINT
echo "./aion --config=mainnet/mainnet.toml \$*">package/$1/mainnet.sh
chmod +x package/$1/mainnet.sh

cp aion/cli/config_testnet_mastery.toml $MASTT
cp core/res/aion/testnet_mastery.json $MASTJ
sed -i '/\<chain = /c chain = \"mastery/mastery.json\"' $MASTT
echo "./aion --config=mastery/mastery.toml \$*">package/$1/mastery.sh
chmod +x package/$1/mastery.sh

cp aion/cli/config_custom.toml $CUSTT
cp core/res/aion/custom.json $CUSTJ
sed -i '/\<chain = /c chain = \"custom/custom.json\"' $CUSTT
echo "./aion --config=custom/custom.toml \$*">package/$1/custom.sh
chmod +x package/$1/custom.sh

tar -C package -czf ${1}.tar.gz $1
echo "Successfully packaged: $(pwd)/${1}.tar.gz !!!"
