CC := gcc
CPP := g++
AR := ar
STRIP := strip
MAKE := make

OUTDIR ?= ../build
TARGET = $(OUTDIR)/libavmjni.a
SHAREDTARGET = $(OUTDIR)/libavmjni.so
DEBUG := -g
OPTIMISE := -O3

SOURCES = $(wildcard *.cpp)
JNIOBJECTS = $(patsubst %.cpp, %.o, $(SOURCES))

JAVA_HOME ?= /usr/lib/jvm/jdk-11.0
JNIINCLUDE := -I. -I${JAVA_HOME}/include -I${JAVA_HOME}/include/linux 

JNIFLAGS := -std=c++0x $(JNIINCLUDE) $(OPTIMISE) -Wall $(DEBUG) -fPIC

.PHONY: all release debug clean $(OUTDIR)

all: release

%.o: %.cpp
	@echo "Compiling $^"
	@$(CPP) $(JNIFLAGS) -c $^ -o $@

debug: $(JNIOBJECTS)
	@echo "Generating $(TARGET)"
	@$(AR) cr $(TARGET) $(JNIOBJECTS)

shared: $(JNIOBJECTS)
	@echo "Generating $(SHAREDTARGET)"
	@$(CPP) $(JNIFLAGS) $^ -shared -o $(SHAREDTARGET)

release: debug shared
	@$(STRIP) -g $(TARGET)

clean:
	@echo "Clean $(JNIOBJECTS)"
	@rm -rf *.o
	@echo "Clean $(TARGET)"
	@rm -rf $(TARGET)
