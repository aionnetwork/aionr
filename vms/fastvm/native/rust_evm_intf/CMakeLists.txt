cmake_minimum_required(VERSION 3.5)

set(CMAKE_CXX_STANDARD 11)

project(rust4evm)

set(AION_CMAKE_DIR  "${CMAKE_CURRENT_LIST_DIR}/cmake")
list(APPEND CMAKE_MODULE_PATH ${AION_CMAKE_DIR})

## fastvm strongly depends on llvm-4.0
option(USE_LLVM "fastvm depends on llvm-4.0 by default" ON)

## modified source code from evmjit
aux_source_directory(libevmjit EVMJIT)

## add library target
add_library(fastvm fastvm.cpp ${EVMJIT})

## add default include dirs
target_include_directories(fastvm PRIVATE . include libevmjit)
## find libLLVM-4.0.so and add its header path
find_package(LLVM)
target_include_directories(fastvm PRIVATE ${LLVM_INCLUDE_DIR} ${LLVM_INCLUDE_C_DIR})

## install to INSTALL_DIR: from cargo now
#install(TARGETS fastvm
#        ARCHIVE DESTINATION ${INSTALL_DIR})
