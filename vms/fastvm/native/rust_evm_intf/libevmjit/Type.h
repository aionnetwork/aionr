#pragma once

#include "preprocessor/llvm_includes_start.h"
#include <llvm/IR/Type.h>
#include <llvm/IR/Constants.h>
#include <llvm/IR/Metadata.h>
#include "preprocessor/llvm_includes_end.h"

#include "JIT.h"

namespace dev
{
namespace eth
{
namespace jit
{
using namespace evmjit;

struct Type
{
	/**
	 * There're only two type of words inside JIT:
	 *
	 * 1) Word: 128-bit integer
	 * 2) Word256: 256-bit integer, to represent both hash and address
	 *
	 * hash = [hash_0_15][hash_16_31]
	 *
	 * address = [address_0_15][address_16_31]
	 */
	static llvm::IntegerType* Word256;
	static llvm::PointerType* Word256Ptr;


	static llvm::IntegerType* Address;
	static llvm::PointerType* AddressPtr;

	static llvm::IntegerType* Word;
	static llvm::PointerType* WordPtr;

	static llvm::IntegerType* Bool;
	static llvm::IntegerType* Size;
	static llvm::IntegerType* Gas;
	static llvm::PointerType* GasPtr;

	static llvm::IntegerType* Byte;
	static llvm::PointerType* BytePtr;

	static llvm::Type* Void;

	/// Main function return type
	static llvm::IntegerType* MainReturn;

	static llvm::PointerType* EnvPtr;
	static llvm::PointerType* RuntimeDataPtr;
	static llvm::PointerType* RuntimePtr;

	// TODO: Redesign static LLVM objects
	static llvm::MDNode* expectTrue;

	static void init(llvm::LLVMContext& _context);
};

struct Constant
{
	static llvm::ConstantInt* gasMax;

	/// Returns word-size constant
	static llvm::ConstantInt* get(int64_t _n);
	static llvm::ConstantInt* get256(int64_t _n);

	static llvm::ConstantInt* get(llvm::APInt const& _n);

	static llvm::ConstantInt* get(ReturnCode _returnCode);
};

}
}
}

