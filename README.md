# AionR

[![Join the chat at https://gitter.im/aionnetwork](https://badges.gitter.im/Join%20Chat.svg)](https://gitter.im/aionnetwork)
[![license](https://img.shields.io/github/license/aionnetwork/aion.svg)](https://github.com/aionnetwork/aionr/blob/dev/LICENSE)
[![contributions welcome](https://img.shields.io/badge/contributions-welcome-brightgreen.svg?style=flat)](https://github.com/aionnetwork/aion/issues)


<img src="aion-rust-logo.png" alt="drawing" width="500"/>

This repository contains the rust kernel implementation and releases for the Aion Network. This is different from the [Java kernel implementation](https://github.com/aionnetwork/aion).

Mainstream adoption of blockchains is limited because of scalability, privacy, and interoperability challenges. Aion is a multi-tier blockchain network designed to address these challenges.

The [Aion White Papers](https://aion.network/developers/#whitepapers) provides more details on our design and project roadmap.

## Install the Kernel

Follow this guide to install the Aion Rust kernel on your system.

### System Requirements

- Ubuntu 16.04 or Ubuntu 18.04
- 4GB RAM
- 2 core CPU
- 60GB Hard Drive Space (Current Mainnet DB about 40GB)

### Prerequisites Installation

1. Update your system and install the build dependencies:

    ```bash
    sudo apt-get update
    sudo apt install g++ gcc libjsoncpp-dev python-dev libudev-dev llvm-4.0-dev cmake wget curl git pkg-config lsb-release -y
    ```

2. Install Rust `v1.28.0`:

    ```bash
    curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain 1.28.0 --default-host x86_64-unknown-linux-gnu
    ```

    Select option `1` when prompted.

3. Initialize the Rust install and check that it is working:

    ```bash
    source $HOME/.cargo/env
    cargo --version

    > cargo 1.28.0 (96a2c7d16 2018-07-13)
    ```

4. Install Boost `v1.65.1` 

    - Ubuntu `16.04`:
    
        ```bash
        wget https://dl.bintray.com/boostorg/release/1.65.1/source/boost_1_65_1.tar.bz2
        tar xf boost_1_65_1.tar.bz2
        cd boost_1_65_1
        ./bootstrap.sh --libdir=/usr/lib/x86_64-linux-gnu/
        ./b2
        ./b2 install
        ```

    - Ubuntu `18.04`:

        ```bash
        sudo apt-get install libboost-filesystem1.65-dev libboost-program-options1.65-dev libboost-regex1.65-dev  -y
        ```

5. Install JAVA JDK:
    * [JDK 11](https://download.java.net/java/GA/jdk11/13/GPL/openjdk-11.0.1_linux-x64_bin.tar.gz) or higher.

6. Install Apache Ant 10:
    * [Apache Ant 10](https://archive.apache.org/dist/ant/binaries/apache-ant-1.10.5-bin.tar.gz)

7. Set Environment Variables:
    ```bash
    export JAVA_HOME=<jdk_directory_location>
    export ANT_HOME=<apache_ant_directory>	
    export LIBRARY_PATH=$JAVA_HOME/lib/server
    export PATH=$PATH:$JAVA_HOME/bin:$ANT_HOME/bin
    export LD_LIBRARY_PATH=$LIBRARY_PATH:/usr/local/lib
    ```
### Build the Kernel

Once you have installed the prerequisites, follow these steps to build the kernel.

1. Download the Aion Rust git repository:

    ```bash
    git clone https://github.com/aionnetwork/aionr.git
    cd aionr
    ```

2. Build the kernel from source:

    ```bash
    ./resources/package.sh aionr-package
    ```

    `aionr-package` is the name that will be given to the Rust package when it as finished building. You can set this to anything you want by changing the last argument in the script call:

    ```bash
    ./resources/package.sh [example-package-name]
    ```

    The package takes about 10 minutes to finish building.

3. When the build has finished, you can find the finished binary at `package/aionr-package`.

## Launch Aion Rust Kernel

1. Navigate to the binary location:

    ```bash
    cd package/aionr-package
    ```

2. Make sure your `JAVA_HOME` is right. :new:

3. Run the `aion` package. Make sure to include any commands you want the kernel to execute. You can find more information on supplying commands in the [user manual](https://github.com/aionnetwork/aionr/wiki/User-Manual#launch-rust-kernel).
Kernel will print **configuration path**, **genesis file path**, **db directory** and **keystore location** at the top of its log.

**We provides quick launch scripts to connect to Mainnet, Mastery and custom network. Running the quick scripts will load the configuration and the genesis in each network folder. You can modify those files in each directory. See launch examples [Kernel Deployment Examples](https://github.com/aionnetwork/aionr/wiki/Kernel-Deployment-Examples)**

```bash
$ ./mainnet.sh

>   ____                 _   _ 
>  / __ \       /\      | \ | |
> | |  | |     /  \     |  \| |
> | |  | |    / /\ \    | . ` |
> | |__| |   / ____ \   | |\  |
>  \____/   /_/    \_\  |_| \_|
>
>
> 2019-11-06 13:54:03        build: Aion(R)/v1.0.0.706f7dc/x86_64-linux-gnu/rustc-1.28.0
> 2019-11-06 13:54:03  config path: kernel_package_path/mainnet/mainnet.toml
> 2019-11-06 13:54:03 genesis path: kernel_package_path/mainnet/mainnet.json
> 2019-11-06 13:54:03    keys path: /home/username/.aion/keys/mainnet
> 2019-11-06 13:54:03      db path: /home/username/.aion/chains/mainnet/db/a98e36807c1b0211
> 2019-11-06 13:54:03      binding: 0.0.0.0:30303
> 2019-11-06 13:54:03      network: Mainnet
> 2019-11-06 13:54:10      genesis: 30793b4ea012c6d3a58c85c5b049962669369807a98e36807c1b02116417f823

```

### Connecting to JSON RPC Services

RPC services can be connected from the following addresses:

- **HTTP**: Port `8545`
- **WebSocket**: Port `8546`
- **IPC**: `$Home/.aion/jsonrpc.ipc`

See the [user manual](https://github.com/aionnetwork/aionr/wiki/User-Manual) or [CMD & Config](https://github.com/aionnetwork/aionr/wiki/CMD-&-Config) wiki to find how to change RPC port settings.

### Miners

If you're interested in mining on the Aion networks, refer to our [Aion Mining Docs](https://docs.aion.network/docs/aion-mining-overview)

### Users

If you're interested in interacting with blockchain applications and _using_ `AION` coins, refer to our [Aion Desktop Wallet Docs](https://docs.aion.network/docs/aion-desktop-wallet).

## Contact

To keep up to date by joining the following channels:

- [Aion Forum](https://forum.aion.network/)  
- [Aion Gitter](https://gitter.im/aionnetwork)  
- [Aion Reddit](https://www.reddit.com/r/AionNetwork/)  
- [Aion Medium](https://blog.aion.network/)

For more information about Aion Community please refer to [Aion Community](https://aion.network/community/)

## License

Aion is released under the [GPL-V3 license](LICENSE)

