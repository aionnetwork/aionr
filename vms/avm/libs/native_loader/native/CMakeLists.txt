cmake_minimum_required(VERSION 3.5)

project(avm_native VERSION 0.2.0)

add_library(avmloader SHARED org_aion_avm_loader_Loader.cpp)
target_include_directories(avmloader PRIVATE .)
target_include_directories(avmloader PUBLIC $ENV{JAVA_HOME}/include $ENV{JAVA_HOME}/include/linux)
