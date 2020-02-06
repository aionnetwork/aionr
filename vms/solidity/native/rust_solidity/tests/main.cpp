#include <evm.h>
#include <evmjit.h>
#include <gtest/gtest.h>
#include <stddef.h>
#include <stdlib.h>
#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <stdexcept>
#include <fstream>

#include <libsolidity/interface/CompilerStack.h>

#include <iostream>
using namespace std;

namespace dev
{
namespace keccak
{
extern void keccak256(uint8_t* output, size_t output_size, const uint8_t* input, size_t input_size);
}
}


struct evm_instance* instance;
struct evm_message msg;

struct evm_address address = { 1, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E };
struct evm_address caller = { 2, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E };
struct evm_word balance = { 0, 0, 0, 0, 0, 0, 0, 0, 0x0D, 0xE0, 0xB6, 0xB3, 0xA7, 0x64, 0x00, 0x00 };
struct evm_word value = { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01, 0, 0, 0, 0 };
struct evm_hash block_hash = { 3   , 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
                               0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e };


struct evm_tx_context tx_context = {
        { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01, 0, 0, 0 }, // tx_gas_price
        { 4, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E }, // tx_origin
        { 5, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E }, // block_coinbase
        16, // block_number
        1501267050506L, // block_timestamp
        1024 * 1024L, // block_gas_limit
        { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01, 0, 0, 0, 0, 0, 0 }, // block_difficulty
};

struct evm_word storage[0x1000000] = {};
bool storage_debug = false;

struct evm_address expected_code_addr = { 6, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E };
uint8_t expected_code_data[] = { 0x11, 0x22, 0x33, 0x44 };

struct evm_word log_topics[8] = {};
size_t log_topics_count = 0;
uint8_t log_data[1024];
size_t log_data_size;


struct evm_message call_msg;
struct evm_address call_output_addr = { 11, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E };
struct evm_word call_output = { 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff};

struct evm_address self_destruct_addr;
struct evm_address self_destruct_bene;

/**
 * evm_account_exists_fn mock
 */
int account_exists(struct evm_context* context,
                   const struct evm_address* address)
{
    if(address->bytes[7] == 0x0f) {
        return 0;
    } else {
        return 1;
    }
}

/**
 * evm_get_balance_fn mock
 */
void get_balance(struct evm_word* result,
                 struct evm_context* context,
                 const struct evm_address* addr)
{
    if (0 == memcmp(address.bytes, addr->bytes, sizeof(evm_address))) {
        *result = balance;
    } else {
        *result = {};
    }
}


/**
 * evm_get_code_fn mock
 */
size_t get_code(const uint8_t** result_code,
                struct evm_context* context,
                const struct evm_address* address)
{
    if (0 == memcmp(&expected_code_addr, address, sizeof(expected_code_addr))) {
        if (result_code)
            *result_code = (uint8_t const*) &expected_code_data;
        return sizeof(expected_code_data);
    } else {
        if (result_code)
            *result_code = nullptr;
        return 0;
    }
}

/**
 * evm_get_storage_fn mock
 */
void get_storage(struct evm_word* result,
                 struct evm_context* context,
                 const struct evm_address* address,
                 const struct evm_word* key)
{
    if (storage_debug) {
        printf("SLOAD: ");
        size_t i = 0;
        for (i = 0; i < sizeof(evm_word); i++) {
            printf("%02x ", key->bytes[i]);
        }
    }

    int x = ((key->bytes[13]) << 16) + ((key->bytes[14]) << 8) + key->bytes[15];
    *result = storage[x];

    if (storage_debug) {
        printf("= ");
        size_t i = 0;
        for (i = 0; i < sizeof(evm_word); i++) {
            printf("%02x ", storage[x].bytes[i]);
        }
        printf("\n");
    }
}

/**
 * evm_set_storage_fn mock
 */
void set_storage(struct evm_context* context,
                 const struct evm_address* address,
                 const struct evm_word* key,
                 const struct evm_word* value)
{
    if (storage_debug) {
        printf("SSTORE: ");
        size_t i = 0;
        for (i = 0; i < sizeof(evm_word); i++) {
            printf("%02x ", key->bytes[i]);
        }
        printf("= ");
        i = 0;
        for (i = 0; i < sizeof(evm_word); i++) {
            printf("%02x ", value->bytes[i]);
        }
        printf("\n");
    }

    int x = ((key->bytes[13]) << 16) + ((key->bytes[14]) << 8) + key->bytes[15];
    storage[x] = *value;
}

/**
 * evm_selfdestruct_fn mock
 */
void selfdestruct(struct evm_context* context,
                  const struct evm_address* address,
                  const struct evm_address* beneficiary)
{
    self_destruct_addr = *address;
    self_destruct_bene = *beneficiary;
}

/**
 * evm_call_fn mock
 */
void call(struct evm_result* result,
          struct evm_context* context,
          const struct evm_message* msg)
{
    call_msg = *msg;

    // memory leak here, but it's fine since we're only testing
    call_msg.input = (const uint8_t*) malloc(msg->input_size);
    memcpy((void*) call_msg.input, msg->input, msg->input_size);

    if(msg->input[14] == 0xfd) {
        result->status_code = EVM_REVERT;
    } else {
        result->status_code = EVM_SUCCESS;
    }

    result->gas_left = msg->gas;


    if (msg->kind == EVM_CREATE) {
        result->output_data = (const uint8_t*) &call_output_addr;
        result->output_size = sizeof(call_output_addr);
    } else {
        result->output_data = (const uint8_t*) &call_output;
        result->output_size = sizeof(call_output);
    }
    result->release = NULL;
    result->reserved.context = NULL;
}

/**
 * evm_get_tx_context_fn mock
 */
void get_tx_context(struct evm_tx_context* result,
                    struct evm_context* context)
{
    memcpy(result, &tx_context, sizeof(evm_tx_context));
}

/**
 * evm_get_block_hash_fn mock
 */
void get_block_hash(struct evm_hash* result,
                    struct evm_context* context,
                    int64_t number)
{
    memcpy(result, &block_hash, sizeof(block_hash));
}

/**
 * evm_log_fn mock
 */
void log(struct evm_context* context,
         const struct evm_address* address,
         const uint8_t* data,
         size_t data_size,
         const struct evm_word topics[],
         size_t topics_count)
{
    log_topics_count = topics_count;
    for (size_t i = 0; i < topics_count; i++) {
        log_topics[i] = topics[i];
    }
    log_data_size = data_size;
    memcpy(log_data, data, min(data_size, sizeof(log_data)));
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

struct evm_context context = { &ctx_fn_table };

int main(int argc, char **argv)
{
    instance = evmjit_create();

    ::testing::InitGoogleTest(&argc, argv);
    return RUN_ALL_TESTS();

    instance->destroy(instance);
}

void setup_message(const uint8_t *code,
                   size_t code_size,
                   const uint8_t *input,
                   size_t input_size,
                   int64_t gas,
                   struct evm_word _value = value)
{
    msg.address = address;
    msg.caller = caller;
    msg.value = _value;
    msg.input = input;
    msg.input_size = input_size;
    dev::keccak::keccak256(msg.code_hash.bytes, 32, code, code_size);
    msg.gas = gas;
    msg.flags = 0;
}

void print_result(struct evm_result *result)
{
    printf("\n  Gas left: %ld\n", result->gas_left);
    printf("  Output size: %zd\n", result->output_size);
    printf("  Output: ");
    size_t i = 0;
    for (i = 0; i < result->output_size; i++) {
        printf("%02x ", result->output_data[i]);
    }
    printf("\n\n");
}

void release_result(struct evm_result *result, bool reset_storage = true)
{
    if (result->release) {
        result->release(result);
    }

    if (reset_storage) {
        memset(&storage, 0, sizeof(storage));
    }
    memset(&log_topics, 0, sizeof(log_topics));
    memset(&log_data, 0, sizeof(log_data));
    memset(&log_data_size, 0, sizeof(log_data_size));
    memset(&call_msg, 0, sizeof(call_msg));
    memset(&self_destruct_addr, 0, sizeof(self_destruct_addr));
    memset(&self_destruct_bene, 0, sizeof(self_destruct_bene));
}

void address2hash(struct evm_address *address, evm_hash *hash)
{
    std::memcpy((uint8_t *)(hash), address, sizeof(evm_address));
}

uint64_t word2uint64(struct evm_word *word)
{
    uint64_t result = 0;
    for (int i = 0; i < 8; i++) {
        result = (result << 8) + (word->bytes[i + 8]);
    }
    return result;
}

int char2int(char input)
{
    if (input >= '0' && input <= '9')
        return input - '0';
    if (input >= 'A' && input <= 'F')
        return input - 'A' + 10;
    if (input >= 'a' && input <= 'f')
        return input - 'a' + 10;
    throw std::invalid_argument("Invalid input string");
}

void hex2bin(const char* src, uint8_t* target)
{
    while(*src && src[1]) {
        *(target++) = char2int(*src)*16 + char2int(src[1]);
        src += 2;
    }
}

string loadContract(string file) {
    std::ifstream t("../../solidity/tests/contracts/" + file);
    std::stringstream buffer;
    buffer << t.rdbuf();

    return buffer.str();
}

void printAsm(string &file, string &name, string &contract, dev::solidity::CompilerStack &compiler) {
    std::map<std::string, std::string> sources;
    sources[file] = contract;
    cout << endl << "EVM assembly:" << endl;
    compiler.streamAssembly(cout, name, sources, false);
    cout << "Binary: " << endl;
    cout << compiler.object(name).toHex() << endl << endl;
}


TEST(test, Basic) {
	storage_debug = true;

    string file = "testBasic.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    uint8_t const input[] = { 0x26, 0x12, 0x1f, 0xf0 };
    int64_t gas = 2000000;
    struct evm_word value = {};

    setup_message(code, sizeof(code), input, sizeof(input), gas, value);
    struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
            code, sizeof(code));
    print_result(&result);

    struct evm_word slot0 = { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01 };
    struct evm_word slot1 = { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02 };
    struct evm_word slot2 = { 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88 };
    struct evm_word slot3 = { 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88 };
    struct evm_word slot4 = { 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88 };
    struct evm_word slot5 = { 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88 };

    ASSERT_TRUE(0 == memcmp(storage[0].bytes, slot0.bytes, sizeof(evm_word)));
    ASSERT_TRUE(0 == memcmp(storage[1].bytes, slot1.bytes, sizeof(evm_word)));
    ASSERT_TRUE(0 == memcmp(storage[2].bytes, slot2.bytes, sizeof(evm_word)));
    ASSERT_TRUE(0 == memcmp(storage[3].bytes, slot3.bytes, sizeof(evm_word)));
    ASSERT_TRUE(0 == memcmp(storage[4].bytes, slot4.bytes, sizeof(evm_word)));
    ASSERT_TRUE(0 == memcmp(storage[5].bytes, slot5.bytes, sizeof(evm_word)));

    release_result(&result);
}

TEST(test, StateVariables1) {
    string file = "testStateVariables.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    uint8_t const input[] = { 0x31, 0x3c, 0xe5, 0x67 }; // decimals()
    int64_t gas = 2000000;
    struct evm_word value = {};

    setup_message(code, sizeof(code), input, sizeof(input), gas, value);
    struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
            code, sizeof(code));
    print_result(&result);

    struct evm_word gt = { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x12 };
    ASSERT_TRUE(0 == memcmp(result.output_data, gt.bytes, sizeof(evm_word)));

    release_result(&result);
}

TEST(test, StateVariables2) {
    string file = "testStateVariables.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    uint8_t const input[] = { 0x06, 0xfd, 0xde, 0x03 }; // name()
    int64_t gas = 2000000;
    struct evm_word value = {};

    setup_message(code, sizeof(code), input, sizeof(input), gas, value);
    struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
            code, sizeof(code));
    print_result(&result);

    string str = "000000000000000000000000000000100000000000000000000000000000001a4578616d706c6520466978656420537570706c7920546f6b656e000000000000";
    uint8_t gt[str.size()/2];
    hex2bin(str.c_str(), gt);

    ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(evm_word)));

    release_result(&result);
}

TEST(test, ArrayResize) {
    string file = "testArrayResize.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    uint8_t const input[] = { 0x26, 0x12, 0x1f, 0xf0 };
    int64_t gas = 2000000;
    struct evm_word value = {};

    setup_message(code, sizeof(code), input, sizeof(input), gas, value);
    struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
            code, sizeof(code));
    print_result(&result);

    struct evm_word slot0 = { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x14 };

    ASSERT_TRUE(0 == memcmp(storage[0].bytes, slot0.bytes, sizeof(evm_word)));

    release_result(&result);
}

TEST(test, ArrayMemory) {
    string file = "testArrayMemory.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    uint8_t const input[] = { 0x82, 0x56, 0xcf, 0xf3,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4 };
    int64_t gas = 2000000;
    struct evm_word value = {};

    setup_message(code, sizeof(code), input, sizeof(input), gas, value);
    struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
            code, sizeof(code));
    print_result(&result);

    string str = "000000000000000000000000000000100000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000200000000000000000000000000000003";
    uint8_t gt[str.size()/2];
    hex2bin(str.c_str(), gt);

    ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

    release_result(&result);
}

TEST(test, ArrayCopy) {
    string file = "testArrayCopy.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    {
        uint8_t const input[] = { 0x26, 0x12, 0x1f, 0xf0 }; // f()
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        string str = "00000000000000000000000000000010000000000000000000000000000000020000000000000000000000000000000100000000000000000000000000000002";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
    {
        uint8_t const input[] = { 0xe2, 0x17, 0x9b, 0x8e }; // g()
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        string str = "000000000000000000000000000000100000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
    {
        uint8_t const input[] = { 0xb8, 0xc9, 0xd3, 0x65 }; // h()
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        string str = "000000000000000000000000000000100000000000000000000000000000000200000000000000000000000001020304050607080910111213141516171819200000000000000000000000002122232425262728293031323334353637383940";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
}

TEST(test, Mappings) {
    storage_debug = true;
    string file = "testMappings.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    uint8_t const input[] = { 0x26, 0x12, 0x1f, 0xf0 };
    int64_t gas = 2000000;
    struct evm_word value = {};

    setup_message(code, sizeof(code), input, sizeof(input), gas, value);
    struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
            code, sizeof(code));
    print_result(&result);

    struct evm_word gt = { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01 };

    ASSERT_TRUE(0 == memcmp(storage[0x5c91e0].bytes, gt.bytes, sizeof(evm_word)));

    release_result(&result);
    storage_debug = false;
}

TEST(test, ExternalFunction) {
	storage_debug = true;
    string file = "testExternalFunction.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    uint8_t const input[] = { 0x26, 0x12, 0x1f, 0xf0};
    int64_t gas = 2000000;
    struct evm_word value = {};

    setup_message(code, sizeof(code), input, sizeof(input), gas, value);
    struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
            code, sizeof(code));
    print_result(&result);

    struct evm_word slot0 = { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xe2, 0x17, 0x9b, 0x8e  };

    ASSERT_TRUE(0 == memcmp(storage[0].bytes, slot0.bytes, sizeof(evm_word)));
    ASSERT_TRUE(0 == memcmp(storage[1].bytes, address.bytes, sizeof(evm_word)));
    ASSERT_TRUE(0 == memcmp(storage[2].bytes, address.bytes + 16, sizeof(evm_word)));

    release_result(&result);
	storage_debug = false;
}


TEST(test, ExternalFunctionCall) {
    string file = "testExternalFunctionCall.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    uint8_t const input[] = { 0x26, 0x12, 0x1f, 0xf0};
    int64_t gas = 2000000;
    struct evm_word value = {};

    setup_message(code, sizeof(code), input, sizeof(input), gas, value);
    struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
            code, sizeof(code));
    print_result(&result);


    uint8_t msg_input[] = { 0xe2, 0x17, 0x9b, 0x8e };

    ASSERT_TRUE(0 == memcmp(call_msg.input, msg_input, sizeof(msg_input)));
    ASSERT_TRUE(0 == memcmp(call_msg.address.bytes, address.bytes, sizeof(evm_address)));

    release_result(&result);
}

TEST(test, FixedBytes) {
    string file = "testFixedBytes.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    {
        string input_str = "a38e374d"; // f(uint128[])
        input_str += "00000000000000000000000000000010"; // data pointer
        input_str += "00000000000000000000000000000002";
        input_str += "00000000000000000000000000000004";
        input_str += "00000000000000000000000000000004";
        uint8_t input[input_str.size()/2];
        hex2bin(input_str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        struct evm_word gt = { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08 };

        ASSERT_EQ(sizeof(evm_word), result.output_size);
        ASSERT_TRUE(0 == memcmp(gt.bytes, result.output_data, sizeof(evm_word)));

        release_result(&result);
    }
    {
        string input_str = "31e9552c"; // g(bytes32[])
        input_str += "00000000000000000000000000000010"; // data pointer
        input_str += "00000000000000000000000000000002";
        input_str += "0000000000000000000000000000000000000000000000000000000000000004";
        input_str += "0000000000000000000000000000000000000000000000000000000000000004";
        uint8_t input[input_str.size()/2];
        hex2bin(input_str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        struct evm_word gt = { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08 };

        ASSERT_EQ(sizeof(evm_word), result.output_size);
        ASSERT_TRUE(0 == memcmp(gt.bytes, result.output_data, sizeof(evm_word)));

        release_result(&result);
    }
}

TEST(test, FixedBytesShift) {
    string file = "testFixedBytesShift.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    {
        uint8_t const input[] = { 0x26, 0x12, 0x1f, 0xf0 }; // f()
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        string str = "3132333435363738000000000000000000000000000000000000000000000000";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(evm_word)));

        release_result(&result);
    }
    {
        uint8_t const input[] = { 0xe2, 0x17, 0x9b, 0x8e }; // g()
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        string str = "2122232425262728313233343536373800000000000000000000000000000000";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(evm_word)));

        release_result(&result);
    }
    {
        uint8_t const input[] = { 0xb8, 0xc9, 0xd3, 0x65 }; // h()
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        string str = "1112131415161718212223242526272831323334353637380000000000000000";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
    {
        uint8_t const input[] = { 0xe5, 0xaa, 0x3d, 0x58 }; // i()
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        string str = "0000000000000000000000000000000000000000000000000102030405060708";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
    {
        uint8_t const input[] = { 0xb5, 0x82, 0xec, 0x5f }; // j()
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        string str = "0000000000000000000000000000000001020304050607081112131415161718";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
    {
        uint8_t const input[] = { 0xb4, 0xf4, 0x0c, 0x61 }; // k()
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        string str = "0000000000000000010203040506070811121314151617182122232425262728";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
}

TEST(test, ERC20) {
    string file = "testERC20.sol";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());
}

TEST(test, AionToken) {
    string file = "testAionToken.sol";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());
}

//========================================
// Examples of the official documentations
//========================================


TEST(docs, LayoutOfSolidity) {
    string file = "docsLayoutOfSolidity.sol";
    string name = "ShapeCalculator";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    string input_hex = "104690b20000000000000000000000000000000300000000000000000000000000000004";
    uint8_t input[input_hex.size()/2];
    hex2bin(input_hex.c_str(), input);

    int64_t gas = 2000000;
    struct evm_word value = {};

    setup_message(code, sizeof(code), input, sizeof(input), gas, value);
    struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
            code, sizeof(code));
    print_result(&result);

    string output_hex = "0000000000000000000000000000000c0000000000000000000000000000000e";
    uint8_t output[output_hex.size()/2];
    hex2bin(output_hex.c_str(), output);

    ASSERT_TRUE(0 == memcmp(result.output_data, output, sizeof(output)));

    release_result(&result);
}

TEST(docs, StructureOfContract) {
    string file = "docsStructureOfContract.sol";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());
}

TEST(docs, Types) {
    string file = "docsValueTypes.sol";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());
}

TEST(docs, GlobalVariables) {
    string file = "docsGlobalVariables.sol";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());
}

TEST(docs, ExpressionsAndFlow) {
    string file = "docsExpressionsAndFlow.sol";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());
}

TEST(docs, Contracts) {
    string file = "docsContracts.sol";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());
}

TEST(docs, Contracts2) {
    string file = "docsContracts2.sol";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());
}

TEST(docs, Contracts3) {
    string file = "docsContracts3.sol";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());
}

TEST(docs, Examples) {
    string file = "docsExamples.sol";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());
}

//========================================
// Extra, focus on ExpressionCompiler
//========================================

TEST(test, StateVariableAccessor) {
    string file = "testStateVariableAccessor.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    {
        uint8_t const input[] = { 0x26, 0x12, 0x1f, 0xf0 }; // f()
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        string str = "00000000000000000000000000000003";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result, false);
    }
    {
        string str = "5765a5cc00000000000000000000000001020304050607080910111213141516171819200000000000000000000000002122232425262728293031323334353637383940";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "00000000000000000000000000000003";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
    {
        string str = "e2179b8e";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(storage[0x1a529c].bytes, gt, sizeof(gt)));

        release_result(&result, false);
    }
    {
        string str = "c6a83d4600000000030000000000000000000000";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
    {
        string str = "b8c9d365";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "00000000000000000000000000000200";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(storage[0xf92ecb].bytes, gt, sizeof(gt)));

        release_result(&result, false);
    }
    {
        string str = "0c6e1a6400000000000000000000000000000001";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "00000000000000000000000000000002";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
    {
        string str = "e5aa3d58";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "0000000000000000000000000000000000000000000000000000000000000002";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result, false);
    }
    {
        string str = "230c695100000000000000000000000000000001";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "0000000000000000000000000000000000000000000000000000000000000002";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
}

TEST(test, UnaryOperation) {
    string file = "testUnaryOperation.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    {
        uint8_t const input[] = { 0x26, 0x12, 0x1f, 0xf0 }; // f()
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        string str = "fefdfcfbfaf9f8f7f6efeeedecebeae9e8e7e6dfdedddcdbdad9d8d7d6cfcecd";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result, false);
    }
}

TEST(test, ComparisonOperation) {
    string file = "testComparisonOperation.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    {
        string str = "26121ff0";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "00000000000000000000000000000001";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
    {
        string str = "e2179b8e";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "00000000000000000000000000000001";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
}

TEST(test, BitOperation) {
    string file = "testBitOperation.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    {
        string str = "26121ff0";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "0102030405060708091011121314151617181900000000000000000000000000";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }

    {
        string str = "e2179b8e";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "2122232425262728293031323334353637383960000000000000000000000000";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }

    {
        string str = "b8c9d365";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "010203040506070809101112131415161718190000000000000000000000000";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
}

TEST(test, FunctionCall) {
    string file = "testFunctionCall.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    {
        string str = "26121ff0";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "1c8aff950685c2ed4bc3174f3472287b56d9517b9c948127319a09a7a36deac8";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
    {
        string str = "e2179b8e";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        // TODO: yulong, add asserts after pre-compiled contracts are enabled

        release_result(&result);
    }
}

TEST(test, IndexAccess) {
    string file = "testIndexAccess.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    {
        string str = "26121ff0";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "6e000000000000000000000000000000";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
    {
        string str = "e2179b8e";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "72000000000000000000000000000000";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
}

TEST(test, Strings) {
    string file = "testStrings.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    {
        string str = "26121ff0";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "000000000000000000000000000000100000000000000000000000000000000c73686f72745f737472696e6700000000";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
    {
        string str = "e2179b8e";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "000000000000000000000000000000100000000000000000000000000000005c766572795f6c6f6e675f737472696e675f616761696e5f616e645f616761696e5f616e645f616761696e5f616e645f616761696e5f616e645f616761696e5f616e645f616761696e5f616e645f616761696e5f616e645f616761696e00000000";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
    {
        string str = "b8c9d365";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "000000000000000000000000000000100000000000000000000000000000000331323300000000000000000000000000";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
    {
        string str = "e5aa3d58";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "6100000000000000000000000000000000000000";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
}


TEST(test, DynamicArray) {
    string file = "testDynamicArray.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    {
        string str = "bacf3d0a0000000000000000000000000000001000000000000000000000000000000003112233445566778811223344556677881122334400000000000000000000000021223344556677881122334455667788112233440000000000000000000000003122334455667788112233445566778811223344000000000000000000000000";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "0000000000000000000000000000001000000000000000000000000000000003112233445566778811223344556677881122334400000000000000000000000021223344556677881122334455667788112233440000000000000000000000003122334455667788112233445566778811223344000000000000000000000000";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
}


TEST(test, Precompiled) {
    string file = "testPrecompiled.sol";
    string name = "Precompiled";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    {
        string str = "51163670";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        release_result(&result);
    }
}


TEST(test, BenchMath) {
    string file = "testBenchMath.sol";
    string name = "Math";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    printAsm(file, name, contract, compiler);
}


TEST(test, BenchToken) {
    string file = "testBenchToken.sol";
    string name = "Token";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    printAsm(file, name, contract, compiler);
}

TEST(test, Ticker) {
    string file = "testTicker.sol";
    string name = "Ticker";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    printAsm(file, name, contract, compiler);
}

TEST(test, Wallet) {
    string file = "testWallet.sol";
    string name = "Wallet";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    printAsm(file, name, contract, compiler);
}

TEST(test, Bancor) {
    string file = "testBancor.sol";
    string name = "BancorQuickConverter";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    printAsm(file, name, contract, compiler);
}

TEST(test, CryptoKittiesCore) {
    string file = "testCryptoKittiesCore.sol";
    string name = "KittyCore";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    printAsm(file, name, contract, compiler);
}

TEST(test, ArrayPush) {
    string file = "testArrayPush.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    {
        string str = "26121ff0";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "00000000000000000000000011223344";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
    {
        string str = "e2179b8e";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "12000000000000000000000000000000";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
}

TEST(test, BlockTransactionProps) {
    storage_debug = true;
    string file = "docsGlobalVariables.sol";
    string name = "BlockTransactionProps";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    string str = "";
    uint8_t input[str.size()/2];
    hex2bin(str.c_str(), input);
    int64_t gas = 2000000;
    struct evm_word value = {};

    setup_message(code, sizeof(code), input, sizeof(input), gas, value);
    struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
            code, sizeof(code));
    print_result(&result);

    struct evm_word zero = {};

    ASSERT_TRUE(0 == memcmp(storage[0x00].bytes, block_hash.bytes, sizeof(evm_word)));
    ASSERT_TRUE(0 == memcmp(storage[0x01].bytes, block_hash.bytes + 16, sizeof(evm_word)));
    ASSERT_TRUE(0 == memcmp(storage[0x02].bytes, tx_context.block_coinbase.bytes, sizeof(evm_word)));
    ASSERT_TRUE(0 == memcmp(storage[0x03].bytes, tx_context.block_coinbase.bytes + 16, sizeof(evm_word)));
    ASSERT_TRUE(0 == memcmp(storage[0x04].bytes, tx_context.block_difficulty.bytes, sizeof(evm_word)));
    ASSERT_EQ(word2uint64(&storage[0x05]), tx_context.block_gas_limit);
    ASSERT_EQ(word2uint64(&storage[0x06]), tx_context.block_number);
    ASSERT_EQ(word2uint64(&storage[0x07]), tx_context.block_timestamp);
    ASSERT_TRUE(0 == memcmp(storage[0x08].bytes, zero.bytes, sizeof(evm_word)));
    // msg.gas check skipped
    ASSERT_TRUE(0 == memcmp(storage[0x0a].bytes, caller.bytes, sizeof(evm_word)));
    ASSERT_TRUE(0 == memcmp(storage[0x0b].bytes, caller.bytes + 16, sizeof(evm_word)));
    // msg.sig check skipped
    ASSERT_TRUE(0 == memcmp(storage[0x0d].bytes, value.bytes, sizeof(evm_word)));
    ASSERT_EQ(word2uint64(&storage[0x0e]), tx_context.block_timestamp);
    ASSERT_TRUE(0 == memcmp(storage[0x0f].bytes, tx_context.tx_gas_price.bytes, sizeof(evm_word)));
    ASSERT_TRUE(0 == memcmp(storage[0x10].bytes, tx_context.tx_origin.bytes, sizeof(evm_word)));
    ASSERT_TRUE(0 == memcmp(storage[0x11].bytes, tx_context.tx_origin.bytes + 16, sizeof(evm_word)));

    release_result(&result);
    storage_debug = false;
}

TEST(test, Array) {
    string file = "testArray.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    {
        string str = "26121ff0";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "0000000000000000000000000000001000000000000000000000000000000028616161616161616161616161616161616161616161616161616161616161616161616161616161610000000000000000";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
    {
        string str = "e2179b8e";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "000000000000000000000000000000100000000000000000000000000000000561616161610000000000000000000000";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
    {
        string str = "53dc9c920000000000000000000000000000001000000000000000000000000000000003000000000000000000000000552233445566778811223344556677881122334400000000000000000000000066223344556677881122334455667788112233440000000000000000000000007722334455667788112233445566778811223344";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);

        str = "0000000000000000000000007722334455667788112233445566778811223344";
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_TRUE(0 == memcmp(result.output_data, gt, sizeof(gt)));

        release_result(&result);
    }
}

TEST(test, Event) {
    string file = "testEvent.sol";
    string name = "Test";
    string contract = loadContract(file);

    dev::solidity::CompilerStack compiler;
    compiler.addSource(file, contract);
    ASSERT_TRUE(compiler.compile());

    // printAsm(file, name, contract, compiler);

    string hex = compiler.object(name).toHex();
    hex = hex.substr(hex.find("60506040", 8));  // run the contract rather deployer
    uint8_t code[hex.length() / 2] = {};
    hex2bin(hex.c_str(), code);

    {
        string str = "26121ff0";
        uint8_t input[str.size()/2];
        hex2bin(str.c_str(), input);
        int64_t gas = 2000000;
        struct evm_word value = {};

        setup_message(code, sizeof(code), input, sizeof(input), gas, value);
        struct evm_result result = instance->execute(instance, &context, EVM_BYZANTIUM, &msg,
                code, sizeof(code));
        print_result(&result);


        string t1 = "a5a5e578255e5ab660d9c29b261345b45717e14f802ba5f52ca064dc4a02bfc3";
        string t2 = "02000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e";
        string t3 = "7465737400000000000000000000000000000000000000000000000000000000";
        string data = "00000000000000000000000000000001";
        str = t1 + t2 + t3 + data;
        uint8_t gt[str.size()/2];
        hex2bin(str.c_str(), gt);

        ASSERT_EQ(3 * 2, log_topics_count);
        ASSERT_TRUE(0 == memcmp(gt + 0, &log_topics[0], sizeof(evm_word)));
        ASSERT_TRUE(0 == memcmp(gt + 16, &log_topics[1], sizeof(evm_word)));
        ASSERT_TRUE(0 == memcmp(gt + 32, &log_topics[2], sizeof(evm_word)));
        ASSERT_TRUE(0 == memcmp(gt + 48, &log_topics[3], sizeof(evm_word)));
        ASSERT_TRUE(0 == memcmp(gt + 64, &log_topics[4], sizeof(evm_word)));
        ASSERT_TRUE(0 == memcmp(gt + 80, &log_topics[5], sizeof(evm_word)));

        ASSERT_EQ(16, log_data_size);
        ASSERT_TRUE(0 == memcmp(gt + 96, log_data, log_data_size));

        release_result(&result);
    }
}
