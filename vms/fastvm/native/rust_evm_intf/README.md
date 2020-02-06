# rust_evm_intf

### Introduction

C interface for Rust ffi, create and run Ethereum Virtual Machine

### Dependencies

- LLVM-4.0

### How to Build

```
$ make; make install
```


if done successfully, libevmjit.so and libfastvm.so will appear in `$HOME/Workspace/Library`. if you run Cargo, 
please add `$HOME/Workspace/Library` into `LIBRARY_PATH` env variable, to let Cargo search link library in this directory.

