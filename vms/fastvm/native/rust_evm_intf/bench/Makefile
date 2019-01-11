.PHONY: all run

SRC := main.cpp
TARGET := fvm

all: fvm

$(TARGET): $(SRC)
	g++ -O3 -I../include $^ -L../dist -lfastvm -L/usr/lib/llvm-4.0 -lLLVM-4.0 -o $@

run:
	export LD_LIBRARY_PATH=../dist; ./$(TARGET)

clean:
	@rm -rf $(TARGET)
