#pragma once

#include "CompilerHelper.h"

namespace dev
{
namespace eth
{
namespace jit
{

class Arith128 : public CompilerHelper
{
public:
	Arith128(IRBuilder& _builder);

	llvm::Value* exp(llvm::Value* _arg1, llvm::Value* _arg2);

	static void debug(llvm::Value *_value, char _c, llvm::Module &_module, IRBuilder &_builder);

	static llvm::Function* getUDiv128Func(llvm::Module& _module);
	static llvm::Function* getURem128Func(llvm::Module& _module);
	static llvm::Function* getURem256Func(llvm::Module& _module);
	static llvm::Function* getUDivRem128Func(llvm::Module& _module);
	static llvm::Function* getSDiv128Func(llvm::Module& _module);
	static llvm::Function* getSRem128Func(llvm::Module& _module);
	static llvm::Function* getSDivRem128Func(llvm::Module& _module);
	static llvm::Function* getUDivRem256Func(llvm::Module& _module);

private:
	llvm::Function* getExpFunc();

	llvm::Function* m_exp = nullptr;
};


}
}
}
