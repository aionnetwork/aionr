LLVM_VERSION = 4.0

UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Linux)
    LLVM_INCLUDE = /usr/include/llvm-$(LLVM_VERSION)
    LLVM_C_INCLUDE = /usr/include/llvm-c-$(LLVM_VERSION)
    LLVM_LIB_DIR = /usr/lib/llvm-$(LLVM_VERSION)/lib
    LLVM_LIB_NAME = LLVM-$(LLVM_VERSION)
    LLVM_LIB = /usr/lib/llvm-4.0/lib/libLLVM-4.0.so
    LINK_SUFFIX = ".so"
endif
ifeq ($(UNAME_S),Darwin)
    LLVM_INCLUDE = /usr/local/opt/llvm@4/include
    LLVM_C_INCLUDE = /usr/local/opt/llvm@4/include/llvm-c
    LLVM_LIB_DIR = /usr/local/opt/llvm@4/lib
    LLVM_LIB_NAME = LLVM
    LLVM_LIB = /usr/local/opt/llvm@4/lib/libLLVM.dylib
    LINK_SUFFIX = ".dylib"
endif



ifeq ($(LLVM_VERSION), 5.0)
LLVM_FLAG := llvm50
else
LLVM_FLAG := llvm40
endif

CC := gcc
CPP := g++
AR := ar
OUTDIR ?= build
DIST = $(OUTDIR)/dist

EVMJIT_SRC = $(wildcard libevmjit/*.cpp)

FASTVM_INC ?= -I.
FASTVM_INC += -I./include -I./libevmjit -I$(LLVM_INCLUDE) -I$(LLVM_C_INCLUDE)

FASTVM_CFLAGS := -std=c++0x $(FASTVM_INC) -O3 -Wall -fPIC $(EVMJIT_LINK_FLAG) -g -fno-builtin
FASTVM_CFLAGS += -Wl,--no-undefined -Wl,-soname=$(EVMJIT_NAME) -D$(LLVM_FLAG)

FASTVM_SRC = $(wildcard *.cpp) $(EVMJIT_SRC)
FASTVM_OBJS = $(patsubst %.cpp, $(OUTDIR)/%.o, $(FASTVM_SRC))
FASTVM_LIB = libfastvm.a

.PHONY: all archive clean $(OUTDIR)

all: archive

$(DIST):
	@mkdir $(DIST)

$(OUTDIR):
	@mkdir -p $(OUTDIR)/libevmjit

$(OUTDIR)/%.o:%.cpp | $(OUTDIR)
	@echo "Compiling $<"
	@$(CPP) $(FASTVM_CFLAGS) -c $^ -static -o $@

archive: $(FASTVM_OBJS) | $(DIST)
	@echo "Generating $(FASTVM_LIB)"
	@echo $(OUTDIR)
	@$(AR) crv $(DIST)/$(FASTVM_LIB) $(FASTVM_OBJS)
	@cp $(LLVM_LIB) $(DIST)
	@chmod +wx $(DIST)/*$(LINK_SUFFIX)

clean:
	@rm -rf $(DIST)/$(FASTVM_LIB) $(FASTVM_OBJS)

distclean:
	@rm -rf $(DIST) $(OUTDIR)