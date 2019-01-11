# AionR

[![version](https://img.shields.io/github/tag/aionnetwork/aionr.svg)](https://github.com/aionnetworkp/aion_rust/releases/latest)
[![Join the chat at https://gitter.im/aionnetwork](https://badges.gitter.im/Join%20Chat.svg)](https://gitter.im/aionnetwork)
[![license](https://img.shields.io/github/license/aionnetwork/aion.svg)](https://github.com/aionnetworkp/aion_rust/blob/dev/LICENSE)
[![contributions welcome](https://img.shields.io/badge/contributions-welcome-brightgreen.svg?style=flat)](https://github.com/aionnetworkp/aion_rust/issues)  

Mainstream adoption of blockchains has been limited because of scalability, privacy, and interoperability challenges. Aion is a multi-tier blockchain network designed to address these challenges. 

Core to our hypothesis is the idea that many blockchains will be created to solve unique business challenges within unique industries. As such, the Aion network is designed to support custom blockchain architectures while providing a trustless mechanism for cross-chain interoperability. 

The [Aion White Papers](https://aion.network/developers/#whitepapers) provides more details regarding our design and project roadmap. 

This repository contains the rust kernel implementation and releases for the Aion Network.

### System Requirements

* **Ubuntu 16.04** or a later version (Recommanded)
* **MacOS 10.12** or a later version

## Getting Started
### Developers

- [Build Rust Kernel](https://github.com/aionnetworkp/aion_rust/wiki/Build-Rust-Kernel) wiki provides the details of building Aion(Rust) Kernel from source code on Ubuntu and MacOS.
- [User Manual](https://github.com/aionnetworkp/aion_rust/wiki/User-Manual) wiki provides the instructions of starting Aion(Rust) Kernel.

#### Requirements
 - Ubuntu
 	
 	Aion rust kernel mainly supports for Ubuntu 16.04 and a later version.
 	- To install required libraries, run
	
	```bash
	sudo apt-get update
	sudo apt install g++ gcc libjsoncpp-dev python-dev libudev-dev llvm-4.0-dev cmake wget curl git pkg-config lsb-release
	```

 	- rustup 1.28.0
	```bash
	curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain 1.28.0 --default-host x86_64-unknown-linux-gnu
	source $HOME/.cargo/env
	# check if rustup installed
	cargo --verions
	```

	- install Boost 1.65.1
	  - For Ubuntu 16.04

		  ```bash
		  wget https://dl.bintray.com/boostorg/release/1.65.1/source/boost_1_65_1.tar.bz2
		  tar xf boost_1_65_1.tar.bz2
		  cd boost_1_65_1
		  ./bootstrap.sh --prefix=/usr/lib/x86_64-linux-gnu/
		  ./b2
		  ./b2 install
		  ```
	  - For Ubuntu 18.04

		  ```bash
		  sudo apt-get install libboost-all-dev
		  ```
	- zmq
	```bash
	sudo apt-get install libzmq3-dev
	```

	- protobuf(Optional)
	  Only if you want to modify wallet protobuf message, then you should install Google Protobuf. Make sure protoc is in PATH environment.
 
 - MacOS: 
   - [User Manual](https://github.com/aionnetworkp/aion_rust/wiki/User-Manual#dependent-libraries) gives the system environment requirements for the binary package. 
   - And [Build Rust Kernel](https://github.com/aionnetworkp/aion_rust/wiki/User-Manual#for-macos) gives system environment requirements for building Aion Rust Kernel from the source code.

#### Build From Source

```bash
# download Aion Rust code
git clone https://github.com/aionnetworkp/aion_rust.git
cd aion_rust

# Build the Kernel
./scripts/package.sh aionr-0.1.0-rc1
```
Executive binary will be found under `./package/aionr-0.1.0-rc1`

#### Launch Aion Rust Kernel

Navigate to `package/aionr-0.1.0-rc1` directory, follow the [Launch Rust Kernel](https://github.com/aionnetworkp/aion_rust/wiki/User-Manual#launch-rust-kernel) section in [User Manual](https://github.com/aionnetworkp/aion_rust/wiki/User-Manual) to Aion Rust Kernel.

### JSON rpc service

RPC service can be connected from:

+	HTTP: port 8545
+	websocket: port 8546
+	ipc: $HOME/.aion/jsonrpc.ipc

Go to [User Manual](https://github.com/aionnetworkp/aion_rust/wiki/User-Manual) or [CMD & Config](https://github.com/aionnetworkp/aion_rust/wiki/CMD-&-Config) wiki to find how to change RPC port settings.

### Miners
If you're interested in mining on the Aion networks, refer to our [Aion Mining Docs](https://docs.aion.network/docs/aion-mining-overview)

### Users
If you're interested in interacting with dApps and _using_ Aion, refer to our [Aion Desktop Wallet Docs](https://docs.aion.network/docs/aion-desktop-wallet)



**Please refer to the [wiki pages](https://github.com/aionnetworkp/aion_rust/wiki) for further documentation and tutorials.**

## Contact

To keep up to date and stay connected with current progress and development, reach out to us on the following channels:

[Aion Forum](https://forum.aion.network/)  
[Aion Gitter](https://gitter.im/aionnetwork)  
[Aion Reddit](https://www.reddit.com/r/AionNetwork/)  
[Aion Medium](https://blog.aion.network/)

For more information about Aion Community please refer to [Aion Community](https://aion.network/community/)

## Join the Team

If you are interested in being part of the Aion project, check out our available positions and apply [here](https://aion.network/careers/)! 

## License

Aion is released under the [GPL-V3 license](https://github.com/aionnetworkp/aion_rust/blob/dev/LICENSE)
