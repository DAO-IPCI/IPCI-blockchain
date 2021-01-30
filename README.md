## Install node

IPCI on Substrate binary blobs available as an assets in [releases](https://github.com/DAO-IPCI/IPCI-blockchain/releases). Latest version you can try to build from source code.

## Building from source

Ensure you have Rust and the support software installed:

```shell
curl https://sh.rustup.rs -sSf | sh
# on Windows download and run rustup-init.exe
# from https://rustup.rs instead

rustup update nightly
rustup target add wasm32-unknown-unknown --toolchain nightly
rustup update stable
cargo +nightly install --git https://github.com/alexcrichton/wasm-gc
```

You will also need to install the following packages:

 - Linux:
```shell
sudo apt install cmake git clang libclang-dev libudev-dev
```
 - Mac:
```shell
brew install cmake pkg-config git llvm
```
 - Windows (PowerShell):

Install LLVM
Download and install the Pre Build Windows binaries
of LLVM  from http://releases.llvm.org/download.html

Install ipci node from git source:
```shell
cargo install --force --git https://github.com/DAO-IPCI/IPCI-blockchain --tag v0.21.0 node-cli
```

Run node in [Robonomics Testnet](https://telemetry.polkadot.io/#/Robonomics):
```shell
ipci
```
Or run your local development network:

```shell
ipci --dev
```

## Network maintaining

