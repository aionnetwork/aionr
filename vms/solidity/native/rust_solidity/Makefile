JSONCPP_INC=/usr/include/jsoncpp

DIST = ./dist

SOLC_INC ?= -I.
SOLC_INC += -I$(JSONCPP_INC)

JULIA_SRC = $(wildcard libjulia/backends/evm/*.cpp)
EVMASM_SRC = $(wildcard libevmasm/*.cpp)
SOLIDITY_SRC = $(wildcard libsolidity/**/*.cpp)
DEVCORE_SRC = $(wildcard libdevcore/*.cpp)

SOLC_SRC = $(wildcard solc/*.cpp) $(JULIA_SRC) $(EVMASM_SRC) $(SOLIDITY_SRC) $(DEVCORE_SRC)
SOLC_OBJS = $(patsubst %.cpp, %.o, $(SOLC_SRC))
SOLC_TARGET = $(DIST)/solc
SOLC_LIB = $(DIST)/libsolc.a

CPP := g++
AR := ar
RM := rm -rf
CFLAGS := -std=c++0x $(SOLC_INC) -O3 -fPIC -g -fno-builtin

.PHONY: all target staticlib clean

all: target 

$(DIST):
	@echo "Creating dist folder $(DIST)"
	@mkdir dist

%.o:%.cpp
	@echo "Compiling $<"
	@$(CPP) $(CFLAGS) -c $^ -o $@

target: $(SOLC_OBJS) | $(DIST)
	@echo "Generating $(SOLC_TARGET)"
	@$(CPP) $(SOLC_OBJS) -Wunknown-pragmas -L/usr/local/lib -lboost_filesystem -lboost_program_options -lboost_regex -lboost_system -ljsoncpp -o $(SOLC_TARGET)

staticlib: $(SOLC_OBJS) | $(DIST)
	@echo "Generating $(SOLC_LIB)"
	@$(AR) crv $(SOLC_LIB) $(SOLC_OBJS)

clean:
	@$(RM) $(DIST)
	@$(RM) $(SOLC_OBJS)
