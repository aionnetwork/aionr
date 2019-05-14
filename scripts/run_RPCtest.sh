#!/bin/bash

set -e

nohup /run/aionminer -l 127.0.0.1:8008 -u 0xa07e185919beef1e0a79fea78fcfabc24927c5067d758e514ad74b905a2bf137 -d 0 -t 1 &


WS="${PWD}"
PACKAGE="aionr-$(git describe --tags)-$(date +%Y%m%d)"

# remove db
echo "clean previous testdb"
rm -rf $HOME/.aion/chains
rm -rf $HOME/.aion/keys/custom

# start kernel to custom network
cd package/"${PACKAGE}"
echo "import accounts"
./custom.sh account import $HOME/.aion/keys/testnet/*

echo "start aion_rust"
nohup  ./custom.sh --author=a07e185919beef1e0a79fea78fcfabc24927c5067d758e514ad74b905a2bf137 &
sleep 7

# go aion_web3_test
cd "${WS}/../aion_web3_test/"
echo "===============start rpc bench==============="
node test_tools/benchtest_web3Requests.js --report "${WS}/test_results/report.html"


echo "===============start rpc test================"
yarn test --detectOpenHandles --runInBand

exit $?
