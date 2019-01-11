#include "Type.h"

#include <llvm/IR/MDBuilder.h>

#include "RuntimeManager.h"

namespace dev
{
namespace eth
{
namespace jit
{

llvm::IntegerType* Type::Word256;
llvm::PointerType* Type::Word256Ptr;
llvm::IntegerType* Type::Address;
llvm::PointerType* Type::AddressPtr;
llvm::IntegerType* Type::Word;
llvm::PointerType* Type::WordPtr;
llvm::IntegerType* Type::Bool;
llvm::IntegerType* Type::Size;
llvm::IntegerType* Type::Gas;
llvm::PointerType* Type::GasPtr;
llvm::IntegerType* Type::Byte;
llvm::PointerType* Type::BytePtr;
llvm::Type* Type::Void;
llvm::IntegerType* Type::MainReturn;
llvm::PointerType* Type::EnvPtr;
llvm::PointerType* Type::RuntimeDataPtr;
llvm::PointerType* Type::RuntimePtr;
llvm::ConstantInt* Constant::gasMax;
llvm::MDNode* Type::expectTrue;

void Type::init(llvm::LLVMContext& _context)
{
	if (!Word)	// Do init only once
	{
		Word256 = llvm::Type::getIntNTy(_context, 256);
		Word256Ptr = Word256->getPointerTo();
		Address = llvm::Type::getIntNTy(_context, 256);
		AddressPtr = Address->getPointerTo();
		Word = llvm::Type::getIntNTy(_context, 128);
		WordPtr = Word->getPointerTo();
		Bool = llvm::Type::getInt1Ty(_context);
		Size = llvm::Type::getInt64Ty(_context);
		Gas = Size;
		GasPtr = Gas->getPointerTo();
		Byte = llvm::Type::getInt8Ty(_context);
		BytePtr = Byte->getPointerTo();
		Void = llvm::Type::getVoidTy(_context);
		MainReturn = llvm::Type::getInt32Ty(_context);

		EnvPtr = llvm::StructType::create(_context, "Env")->getPointerTo();
		RuntimeDataPtr = RuntimeManager::getRuntimeDataType()->getPointerTo();
		RuntimePtr = RuntimeManager::getRuntimeType()->getPointerTo();

		Constant::gasMax = llvm::ConstantInt::getSigned(Type::Gas, std::numeric_limits<int64_t>::max());

		expectTrue = llvm::MDBuilder{_context}.createBranchWeights(1, 0);
	}
}

llvm::ConstantInt* Constant::get(int64_t _n)
{
	return llvm::ConstantInt::getSigned(Type::Word, _n);
}

llvm::ConstantInt* Constant::get256(int64_t _n)
{
	return llvm::ConstantInt::getSigned(Type::Word256, _n);
}

llvm::ConstantInt* Constant::get(llvm::APInt const& _n)
{
	return llvm::ConstantInt::get(Type::Word->getContext(), _n);
}

llvm::ConstantInt* Constant::get(ReturnCode _returnCode)
{
	return llvm::ConstantInt::get(Type::MainReturn, static_cast<uint64_t>(_returnCode));
}

}
}
}

