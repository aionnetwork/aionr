#include <ctime>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <inttypes.h>

#include <evm.h>
#include <evmjit.h>

static int account_exists(
        struct evm_context* context,
        const struct evm_address* address)
{
    return 0;
}

static void get_balance(
        struct evm_word* result,
        struct evm_context* context,
        const struct evm_address* addr)
{
}

static size_t get_code(
        const uint8_t** result_code,
        struct evm_context* context,
        const struct evm_address* address)
{
    return 0;
}

static void get_storage(
        struct evm_word* result,
        struct evm_context* context,
        const struct evm_address* address,
        const struct evm_word* key)
{
}

static void set_storage(
        struct evm_context* context,
        const struct evm_address* address,
        const struct evm_word* key,
        const struct evm_word* value)
{
}

static void selfdestruct(
        struct evm_context* context,
        const struct evm_address* address,
        const struct evm_address* beneficiary)
{
}

static void call(
        struct evm_result* result,
        struct evm_context* context,
        const struct evm_message* msg)
{
}

static void get_tx_context(
        struct evm_tx_context* result,
        struct evm_context* context)
{
}

static void get_block_hash(
        struct evm_hash* result,
        struct evm_context* context,
        int64_t number)
{
}

static void log(
        struct evm_context* context,
        const struct evm_address* address,
        const uint8_t* data,
        size_t data_size,
        const struct evm_word topics[],
        size_t topics_count)
{
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

int main(int argc, char **argv) {
    // create a vm instance
    struct evm_instance* instance = evmjit_create();

    // prepare the code (compiled by the provided compiler)
    uint8_t const code[] = {
            0x60, 0x00, // push i

            0x5b,
            0x80, // copy i
            0x61, 0x04, 0x00, // push 1024
            0x10, // 1024 < i
            0x60, 0x19, 0x57, // jump if true

            0x80, // copy i
            0x60, 0xE0, 0x51, // mload sum
            0x01, // sum += i
            0x60, 0xE0, 0x52, // mstore sum
            0x60, 0x01, // push 1
            0x01, // i += 1
            0x60, 0x02, 0x56, // jump

            0x5b,
            0x60, 0x10, 0x60, 0xE0, 0xF3 // RETURN
    };
    const size_t code_size = sizeof(code);
    struct evm_hash code_hash = {
            1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5, 6, 7, 8,
            1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5, 6, 7, 8
    };

    // prepare the input, value, and gas
    uint8_t const input[] = {};
    struct evm_word value = { };
    int64_t gas = 5000000;

    // prepare the context and message
    struct evm_context ctx = { &ctx_fn_table };
    struct evm_address address = {
            1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5, 6, 7, 8,
            1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5, 6, 7, 8
    };
    struct evm_address caller = {
            1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5, 6, 7, 8,
            1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5, 6, 7, 8
    };
    struct evm_message msg = {
            address,
            caller,
            value,
            input,
            sizeof(input),
            code_hash,
            gas,
            0
    };

    // compile once
    struct evm_result result = instance->execute(instance, &ctx, EVM_AION, &msg, code, code_size);
    printf("Energy used: %" PRId64 "\n", gas - result.gas_left);
    printf("Energy left: %" PRId64 "\n", result.gas_left);
    printf("Output size: %zd\n", result.output_size);
    printf("Output: ");
    size_t i = 0;
    for (i = 0; i < result.output_size; i++) {
        printf("%02x ", result.output_data[i]);
    }
    printf("\n");
    if (result.release) {
        result.release(&result);
    }

    // benchmark
    int repeat = 1000;
    clock_t begin = clock();
    for (int i = 0; i < repeat; i++) {
        // run the vm
        result = instance->execute(
                instance,
                &ctx,
                EVM_AION,
                &msg,
                code,
                code_size
        );

        // release resources
        if (result.release) {
            result.release(&result);
        }
    }
    clock_t end = clock();
    printf("Time elapsed: %zd μs per execution\n", 1000000 * (end - begin) / repeat / CLOCKS_PER_SEC);

    // destroy the vm
    instance->destroy(instance);

    return 0;
}
