#ifndef __FASTVM_H_
#define __FASTVM_H_

#include <stdio.h>
#include <string.h>

#if defined(__MACH__)
#include <stdlib.h>
#else
#include <malloc.h>
#endif

#include "evmjit.h"

#if __cplusplus
extern "C" {
#endif

#define DEBUG   0
#define CHECK_STRUCT_ALIGN    (0)
#define DUMP_INSTANCE         (0)
#define DUMP_CODE_INFO        (0)
#define DUMP_CONTEXT          (0)
#define DUMP_VM_INFO          (0)
#define DUMP_EVM_RESULT       (0)
#define DUMP_RET_DATA         (0)
#define DUMP_RET_RESULT       (0)

struct parity_msg {
    struct evm_address recv_addr;
    struct evm_message msg;
} ;

struct code_info {
    uint32_t code_size;
    uint8_t *code_ptr;
};

struct result_info {
    enum evm_status_code status;
    long gas_left;
    uint8_t *output_data;
    size_t  output_size;
};

#define debug(args...)                          \
  if (DEBUG) printf(args);

  typedef int (*exists_cb)(const void *obj, const struct evm_address *address);
  typedef uint8_t *(*get_storage_cb)(const void *obj, const struct evm_address *address,
                                 const struct evm_word *key);
  typedef void (*put_storage_cb)(const void *obj, const struct evm_address *address,
                                 const struct evm_word *key,
                                 const struct evm_word *value);
  typedef uint8_t *(*get_balance_cb)(const void *obj, const struct evm_address *address);
  typedef void (*get_code_cb)(const void *obj, struct code_info *info,
                                const struct evm_address *address);
  typedef void (*selfdestruct_cb)(const void *obj, const struct evm_address *address,
                                  const struct evm_address *beneficiary);
  typedef uint8_t *(*call_cb)(const void *obj, struct result_info *info,
                          const struct parity_msg *msg);
  typedef void (*get_tx_context_cb)(const void *obj, struct evm_tx_context *result);
  typedef uint8_t *(*get_blockhash_cb)(const void *obj, int64_t number);
  typedef void (*log_cb)(const void *obj, const struct evm_address *address,
                         const uint8_t *data,
                         size_t data_size,
                         const struct evm_word topics[],
                         size_t topics_count);

  typedef struct EvmCBS {
    exists_cb exists;
    get_storage_cb get_storage;
    put_storage_cb put_storage;
    get_balance_cb get_balance;
    get_code_cb get_code;
    selfdestruct_cb selfdestruct;
    call_cb call;
    get_tx_context_cb get_tx_context;
    get_blockhash_cb get_blockhash;
    log_cb log;
  } EVM_CBS;

  extern void do_keccak(uint8_t const *data, uint64_t size, uint8_t *o_hash);
  void trigger_cb(void);
  void register_callback(void (*func)());

#define DBG_DUMP_RESULT         \
    debug("\n\n====================== Result ======================\n");        \
    debug("status code : %d\n", evm_result.status_code);            \
    debug("gas_left : %ld\n", evm_result.gas_left);                 \
    debug("output_size : %ld\n", evm_result.output_size);            \
    debug("output data : ");                \
    for (idx = 0; idx < evm_result.output_size; idx++) {        \
      debug("%d ", evm_result.output_data[idx]);            \
    }           \
    debug("\n");

#define DBG_DUMP_RET_RESULT         \
    debug("=================== send back result =======================\n");    \
    debug("status : %d\n", result->status_code);        \
    debug("gas left : %ld\n", result->gas_left);        \
    debug("output size : %ld\n", result->output_size);  \
    debug("output data at %p: \n", result->output_data);    \
    for (idx = 0; idx < result->output_size; idx++) {       \
      debug("%d ", result->output_data[idx]);           \
    }       \
    debug("\n");

#define DBG_DUMP_CODE_INFO      \
    debug("Code Info\n");       \
    for (idx = 0; idx < len; printf("%02x ", code[idx]), idx++);

#define DBG_DUMP_VM_INFO        \
    debug("================= Virtual Machine Message Info =======================\n");      \
    debug("address: ");     \
    for (idx = 0; idx < 32; idx++) {debug("%d ", msg.address.bytes[idx]);}debug("\n");  \
    debug("caller: ");      \
    for (idx = 0; idx < 16; idx++) {debug("%d ", msg.caller.bytes[idx]);}debug("\n");   \
    debug("value: ");       \
    for (idx = 0; idx < 16; idx++) {debug("%d ", msg.value.bytes[idx]);}debug("\n");    \
    debug("input size: %d\n", msg.input_size);      \
    debug("\n\n");      \
    debug("================= Virtual Machine Execution Context Info =======================\n");    \
    debug("tx gas price: ");    \
    for (idx = 0; idx < 16; idx++) {debug("%d ", ctx.tx_gas_price.bytes[idx]);}debug("\n");     \
    debug("tx origin: ");   \
    for (idx = 0; idx < 32; idx++) {debug("%d ", ctx.tx_origin.bytes[idx]);}debug("\n");        \
    debug("block coin base: ");     \
    for (idx = 0; idx < 32; idx++) {debug("%d ", ctx.block_coinbase.bytes[idx]);}       \
    debug("\n");    \
    debug("block number: %ld\n", ctx.block_number);     \
    debug("block timestamp: %ld\n", ctx.block_timestamp);       \
    debug("block gas limit: %ld\n", ctx.block_gas_limit);       \
    debug("block difficulty: ");        \
    for (idx = 0; idx < 16; idx++) {debug("%d ", ctx.block_difficulty.bytes[idx]);}debug("\n");


#if __cplusplus
}
#endif

#endif
