#!/bin/bash
PACKAGE_NAME=$1
echo $PACKAGE_NAME

mkdir executive || echo "executive folder exist"
cp -r ../package/$PACKAGE_NAME executive/

# download openjdk-11.0.1
JDK_TAR="openjdk-11.0.1_linux-x64_bin.tar.gz"
JDK_SHA1SUM=2737d3c1c67d5629383d6da4c4c33b1e3427c3d6

if [ ! -e "${JDK_TAR}" ];then
    wget https://download.java.net/java/GA/jdk11/13/GPL/openjdk-11.0.1_linux-x64_bin.tar.gz
fi

if [ "`sha1sum $JDK_TAR | awk '{print$1}'`" != "$JDK_SHA1SUM" ];then
    echo "Broken OpenJDK"
    exit -1
fi

mkdir libs || echo "libs folder exists"
tar xvf ${JDK_TAR} -C libs

docker build --file Dockerfile --build-arg PACKAGE_LOCATION=executive/$PACKAGE_NAME -t aionr .

# rm -rf executive
