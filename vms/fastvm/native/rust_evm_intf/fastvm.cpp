#include "fastvm.h"
#include <assert.h>

#define enter() debug("enter %s\n", __func__);

/**
 * free result
 */
void release_result(const struct evm_result *result)
{
    free(result->reserved.context);
}

/*
 * util funcs
 */
static long read_long(void *addr)
{
  return *(long *)addr;
}

static int read_int(void *addr)
{
  return *(int *)addr;
}

static void write_int(void *addr, int value)
{
  *(int *)addr = value;
}

static void write_long(void *addr, long value)
{
  *(long *)addr = value;
}

#if __cplusplus
extern "C" {
#endif

  struct evm_tx_context ctx;
  //static const void *gbl_cb_obj;
  #define CALL_MAX_DEPTH    1024
  static void *gbl_cb_objs[CALL_MAX_DEPTH];
  static int cur_obj = -1;

  static EVM_CBS evm_cbs_s = {NULL};
  static EVM_CBS *evm_cbs_p = &evm_cbs_s;

  void *pop_gbl_obj()
  {
    if (cur_obj >= 0) {
      cur_obj -= 1;
      return gbl_cb_objs[cur_obj+1];
    }

    return NULL;
  }

  void push_gbl_obj(void *gbl_obj)
  {
    assert(cur_obj < CALL_MAX_DEPTH);
    cur_obj += 1;
    gbl_cb_objs[cur_obj] = gbl_obj;
  }

  static void *gbl_cb_obj = NULL;

  // do receive address management
  static struct evm_address recv_addr_repo[CALL_MAX_DEPTH] = {0};
  static struct curr_recv {
    struct evm_address addr;
    int filled;
  } curr_recv_addr = {0};

  static int recv_idx = -1;

  /**
   * evm_account_exists_fn
   */
  static int account_exists(struct evm_context *context,
                     const struct evm_address *address)
  {
    const void *obj  = gbl_cb_obj;

    debug("sizeof(int) = %ld, sizeof(size_t) = %ld\n", sizeof(int), sizeof(size_t));
    enter();
    if (NULL != evm_cbs_p->exists) {
      int ret = evm_cbs_p->exists(obj, *address);
      return ret;
    }
    return 0;
  }

  /**
   * evm_get_storage_fn
   */
  static void get_storage(struct evm_word *result,
                   struct evm_context *context,
                   const struct evm_address *address,
                   const struct evm_word *key)
  {
    const void *obj = gbl_cb_obj;
    (void)context;
    enter();

    if (NULL != evm_cbs_p->get_storage) {
      struct evm_word value = evm_cbs_p->get_storage(obj, *address, *key);
      memcpy(result->bytes, value.bytes, sizeof(evm_word));
    }
  }

  /**
   * evm_set_storage_fn
   */
  static void set_storage(struct evm_context *context,
                   const struct evm_address *address,
                   const struct evm_word *key,
                   const struct evm_word *value)
  {
    (void)context;
    enter();
    if (NULL != evm_cbs_p->put_storage)
      evm_cbs_p->put_storage(gbl_cb_obj, *address, *key, *value);
  }

  /**
   * evm_get_balance_fn
   */
  static void get_balance(struct evm_word *result,
                   struct evm_context *context,
                   const struct evm_address *address)
  {
    (void)context;
    enter();
    
    if (NULL != evm_cbs_p->get_balance) {
      struct evm_word balance = evm_cbs_p->get_balance(gbl_cb_obj, *address);
      memcpy(result->bytes, balance.bytes, sizeof(evm_word));
    }
  }

  /**
   * evm_get_code_fn
   */
  static size_t get_code(const uint8_t **result_code,
                  struct evm_context *context,
                  const struct evm_address *address)
  {
    (void)context;
    enter();

    struct code_info info = {0};

    if (NULL != evm_cbs_p->get_code) {
        evm_cbs_p->get_code(gbl_cb_obj, &info, *address);

        if (result_code) {
            uint8_t *code_ptr = (uint8_t *)malloc(info.code_size);
            memcpy(code_ptr, info.code_ptr, info.code_size);
            *result_code = code_ptr;
        }
    }

    return info.code_size;
  }

  /**
   * evm_selfdestruct_fn
   */
  static void selfdestruct(struct evm_context *context,
                    const struct evm_address *address,
                    const struct evm_address *beneficiary)
  {
    (void)context;
    enter();
    if (NULL != evm_cbs_p->selfdestruct)
      evm_cbs_p->selfdestruct(gbl_cb_obj, *address, *beneficiary);
  }

  /**
   * evm_call_fn
   */
  static void call(struct evm_result *result,
            struct evm_context *context,
            const struct evm_message *msg)
  {
    (void)context;
    enter();
    uint8_t *output_ptr = NULL;

    struct result_info info = {EVM_SUCCESS, 0, NULL, 0};

    // needs to tell parity the receive address
    struct parity_msg _msg;

    if (msg->kind != EVM_CALLCODE && msg->kind != EVM_DELEGATECALL) {
      memcpy(&curr_recv_addr.addr, &msg->address, sizeof(evm_address));
    }

    memcpy(&_msg.msg, msg, sizeof(evm_message));
    memcpy(&_msg.recv_addr, &curr_recv_addr.addr, sizeof(evm_address));

    if (NULL != evm_cbs_p->call)
      output_ptr = evm_cbs_p->call(gbl_cb_obj, &info, &_msg);

    result->status_code = info.status;
    result->gas_left = info.gas_left;
    result->output_size = info.output_size;

    uint8_t *buf = (uint8_t *)malloc(info.output_size);
    memcpy(buf, output_ptr, info.output_size);;
    free(output_ptr);

    result->output_data = buf;

    debug("\ncall status code = %d\n", info.status);
    debug("call gas_left = %ld\n", info.gas_left);
    debug("call output_size = %ld\n", info.output_size);

    result->release = &release_result;
    result->reserved.context = buf;
  }

  /**
   * evm_get_tx_context_fn
   */
  static void get_tx_context(struct evm_tx_context *result,
                      struct evm_context *context)
  {
    enter();
    if ((NULL != result) && (NULL != context)) {
      memcpy(result, &ctx, sizeof(struct evm_tx_context));
    } else {
      debug("Please check result and context\n");
    }
  }

  /**
   * evm_get_block_hash_fn
   */
  static void get_block_hash(struct evm_hash *result,
                      struct evm_context *context,
                      int64_t number)
  {
    (void)context;
    
    enter();

    if (NULL != evm_cbs_p->get_blockhash) {
      struct evm_hash block_hash = evm_cbs_p->get_blockhash(gbl_cb_obj, number);
      memcpy(result->bytes, block_hash.bytes, sizeof(evm_hash));
    }
  }

  /**
   * evm_log_fn
   */
  static void log(struct evm_context *context,
           const struct evm_address *address,
           const uint8_t *data,
           size_t data_size,
           const struct evm_word topics[],
           size_t topics_count)
  {
    (void)context;

    enter();
    if (NULL != evm_cbs_p->log) {
        evm_cbs_p->log(gbl_cb_obj, *address, data, data_size, topics, topics_count);
    }
  }

  static const struct evm_context_fn_table ctx_fn_table = {
    account_exists,
    get_storage,
    set_storage,
    get_balance,
    get_code,
    selfdestruct,
    call,
    get_tx_context,
    get_block_hash,
    log
  };

  struct evm_context vm_context = { &ctx_fn_table };


  void *fastvm_create()
  {
    struct evm_instance *instance = evmjit_create();
    return instance;
  }

  struct rust_vm_result {
    char status_code;
    int64_t gas_left;
    uint8_t *output_data;
    size_t output_size;
  };

  int env_init(void *cb_obj)
  {
    push_gbl_obj(cb_obj);
    gbl_cb_obj = cb_obj;

    return 0;
  }

  uint8_t *vm_alloc_data(int32_t size) {
    return (uint8_t *)malloc(size);
  }

  void parse_context(char *b, struct evm_message *msg, struct evm_tx_context *ctx)
  {
    unsigned address_len = 32;
    unsigned offset = 0;
    memcpy(msg->address.bytes, b + offset, address_len); offset += address_len; // address
    memcpy(ctx->tx_origin.bytes, b + offset, address_len); offset += address_len; // origin
    memcpy(msg->caller.bytes, b + offset, address_len); offset += address_len; // caller
    offset += 16; // gas price = 16 bytes
    msg->gas = read_long(b + offset); offset += 8; // gas limit
    memcpy(msg->value.bytes, b + offset, 16); offset += 16; // call value

    msg->input_size = read_int(b + offset); offset += 4;

    debug("message input size = %ld\n", msg->input_size);

    msg->input = (const unsigned char*)(b + offset); offset += msg->input_size; // call data

    msg->depth = read_int(b + offset); offset += 4; // depth

    msg->kind = (enum evm_call_kind)read_int(b + offset); offset += 4; // kind

    msg->flags = read_int(b + offset); offset += 4; // flags

    memcpy(ctx->block_coinbase.bytes, b + offset, address_len); offset += address_len; // block coinbase

    ctx->block_number = read_long(b + offset); offset += 8; // block number

    ctx->block_timestamp = read_long(b + offset); offset += 8; // block timestamp

    ctx->block_gas_limit = read_long(b + offset); offset += 8; // block gas limit

    memcpy(ctx->block_difficulty.bytes, b + offset, 16); offset += 16; // call value
  }

  /*
   * inst: Ethereum Virtual Machine instance
   * code: code run on EVM
   * len: code length
   * context: running context for EVM
   * size: return data length
   */
  int fastvm_run(struct evm_instance *inst, const unsigned char code[], const unsigned len, char context[], int rev, struct rust_vm_result *result)
  {
    uint32_t idx = 0;
    
    struct evm_message msg;
    parse_context(context, &msg, &ctx);

    // push receive address
    if (curr_recv_addr.filled == 0) {
      memcpy(&curr_recv_addr.addr, &msg.address, sizeof(evm_address));
      curr_recv_addr.filled = 1;
    }
    recv_idx += 1;
    memcpy(&recv_addr_repo[recv_idx], &curr_recv_addr.addr, sizeof(evm_address));

    do_keccak((const uint8_t*) code, len, msg.code_hash.bytes);
    struct evm_result evm_result = inst->execute(inst, &vm_context, (enum evm_revision)rev, &msg,
                                                 (uint8_t *)code, len);

    result->status_code = evm_result.status_code;
    result->gas_left = evm_result.gas_left;
    result->output_size = evm_result.output_size;
    debug("evm execution result's gas left = %d\n", result->gas_left);
    for (idx = 0; idx < result->output_size; idx++) {
      result->output_data[idx] = evm_result.output_data[idx];
    }

    pop_gbl_obj();
    gbl_cb_obj = gbl_cb_objs[cur_obj];
    recv_idx -= 1;
    if (recv_idx == -1)
      curr_recv_addr.filled = 0;

    if (evm_result.release) {
      evm_result.release(&evm_result);
    }
    return 0;
  }

#if __cplusplus
}
#endif


/**
 * Encodes execution result.
 */
void encode_result(const struct evm_result *evm_result, struct evm_result result)
{
  unsigned size = 4 + 8 + 4 + evm_result->output_size;
  char *buf = (char *)malloc(size);

  unsigned offset = 0;
  write_int(buf + offset, evm_result->status_code); offset += 4; // code
  write_long(buf + offset, evm_result->gas_left); offset += 8; // gas left
  write_int(buf + offset, evm_result->output_size); offset += 4; // output size
  memcpy(buf + offset, evm_result->output_data, evm_result->output_size); offset += evm_result->output_size; // output
}


#if __cplusplus
extern "C" {
#endif

static void (*test_func)() = NULL;

void register_callback(void (*func)())
{
  test_func = func;
}
  
#define register_cb_fn(type, name)                                  \
  void register_##name##_fn(type name) {evm_cbs_p->name = name;}

  register_cb_fn(exists_cb, exists);
  register_cb_fn(get_storage_cb, get_storage);
  register_cb_fn(put_storage_cb, put_storage);
  register_cb_fn(get_balance_cb, get_balance);
  register_cb_fn(get_code_cb, get_code);
  register_cb_fn(selfdestruct_cb, selfdestruct);
  register_cb_fn(call_cb, call);
  register_cb_fn(get_tx_context_cb, get_tx_context);
  register_cb_fn(get_blockhash_cb, get_blockhash);
  register_cb_fn(log_cb, log);
#if __cplusplus
}
#endif