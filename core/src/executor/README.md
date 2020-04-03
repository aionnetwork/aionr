# What is executor?

An executor is responsible for managing VM execution resources, including runtime context, return data, runtime status. For Aion, it is specifically for AVM (an java based Virtual Machine), and the 'old' fastvm.

## fastvm executor

fastvm tries to execute every transaction by sequence. Its *transact* method accepts one transaction, does some sanity checks and generates context for this transaction before vm starts running.

## avm executor

avm always tries to execute transactions concurrently, whether it's really concurrency or not. Concurrency info can not be detected in the kernel, so just send transactions to avm with proper context. Avm will do all the work for us.