#ifndef AVM_H
#define AVM_h

#include <stddef.h>
#include <stdint.h>
#include <malloc.h>
#include <string.h>

#if __cplusplus
extern "C" {
#endif

typedef int8_t   i8;
typedef int16_t  i16;
typedef int32_t  i32;
typedef int64_t  i64;
typedef uint8_t  u8;
typedef uint16_t u16;
typedef uint32_t u32;
typedef uint64_t u64;

/**
 * Represents a 32-bytes Aion address.
 */
struct avm_address
{
    u8 bytes[32];
};

/**
 * Represents a value in Aion. Using fixed bytes, instead of bigInt, for convenience.
 */
struct avm_value
{
    u8 bytes[32];
};

/**
 * Represents a byte array in the heap.
 */
struct avm_bytes
{
    u32 length; // the array length
    u8 *pointer;  // the memory address of the array, or NULL
};

/**
 * Create account callback function.
 *
 * Creates a new account state for the given address if it does not exist.
 */
typedef void (*avm_create_account_fn)(const void *handle, const struct avm_address *address);

/**
 * Check account exists callback function.
 *
 * Returns 1 if the account exists, otherwise 0.
 */
typedef u32 (*avm_has_account_state_fn)(const void *handle, const struct avm_address *address);

/**
 * Put code callback function.
 *
 * Sets the code of an account; the client is responsible for releasing the allocated memory
 * for storing the code.
 */
typedef void (*avm_put_code_fn)(const void *handle, const struct avm_address *address,
    const struct avm_bytes *code);

/**
 * Get code callback function.
 *
 * Returns the code of an account.
 */
typedef struct avm_bytes (*avm_get_code_fn)(const void *handle, const struct avm_address *address);

/**
 * Put storage callback function.
 *
 * Adds a key-value storage item into the given account's storage.
 */
typedef void (*avm_put_storage_fn)(const void *handle, const struct avm_address *address,
    const struct avm_bytes *key, const struct avm_bytes *value);

/**
 * Get storage callback function.
 *
 * Returns the value which is associated with the given key, at the specified account.
 */
typedef struct avm_bytes (*avm_get_storage_fn)(const void *handle, const struct avm_address *address,
    const struct avm_bytes *key);

/**
 * Account self-destruct callback function.
 *
 * Remove the account state. TODO: specification
 */
typedef void (*avm_delete_account_fn)(const void *handle, const struct avm_address *address);

/**
 * Get account balance callback function.
 *
 * Returns the balance of the given account into the provided buffer.
 */
typedef struct avm_value (*avm_get_balance_fn)(const void *handle, const struct avm_address *address);

/**
 * Increase balance callback function.
 *
 * Increase the balance of the given account.
 */
typedef void (*avm_increase_balance_fn)(const void *handle, const struct avm_address *address,
    const struct avm_value *value);

/**
 * Decrease balance callback function.
 *
 * Decrease the balance of the given account.
 */
typedef void (*avm_decrease_balance_fn)(const void *handle, const struct avm_address *address,
    const struct avm_value *value);

/**
 * Get the address nonce callback function.
 *
 * Returns the current nonce of the given account.
 */
typedef u64 (*avm_get_nonce_fn)(const void *handle, const struct avm_address *address);

/**
 * Increment nonce callback function.
 *
 * Increase the account nonce by 1.
 */
typedef void (*avm_increment_nonce_fn)(const void *handle, const struct avm_address *address);

/*
 * touch accounts in substate to help generate properiate state root of each transaction
 */
typedef void (*avm_touch_account_fn)(const void *handle, const struct avm_address *address, const i32 idx);

typedef struct avm_bytes (*avm_send_signal_fn)(const void *handle, const i32 sig_num);

typedef struct avm_bytes (*avm_contract_address_fn)(const struct avm_address *address, const struct avm_bytes *nonce);

typedef void (*avm_add_log_fn)(const void *handle, const struct avm_bytes *log, const i32 idx);

typedef struct avm_bytes (*avm_get_transformed_code_fn)(
    const void *handle,
    const struct avm_address *address,
    const u8 version);

typedef void (*avm_set_objectgraph_fn)(const void *handle, const struct avm_address *address, const struct avm_bytes *data);

typedef struct avm_bytes (*avm_get_objectgraph_fn)(const void *handle, const struct avm_address *address);

typedef void (*avm_set_transformed_code_fn)(
    const void *handle,
    const struct avm_address *address,
    const struct avm_bytes *data,
    const u8 version);

typedef struct avm_bytes (*avm_get_blockhash_fn)(const void *handle, const i64 block_number);

typedef struct avm_bytes (*avm_sha256_fn)(const struct avm_bytes *data);

typedef struct avm_bytes (*avm_blake2b_fn)(const struct avm_bytes *data);

typedef struct avm_bytes (*avm_keccak256_fn)(const struct avm_bytes *data);

typedef bool (*avm_edveryfy_fn)(const struct avm_bytes *data, const struct avm_bytes *data1, const struct avm_bytes *data2);

typedef void (*avm_remove_storage_fn)(const void *handle, const struct avm_address *address, const struct avm_bytes *data);

typedef bool (*avm_has_storage_fn)(const void *handle, const struct avm_address *address);

/**
 * A data structure holds all the callback function pointers.
 */
struct avm_callbacks {
    avm_create_account_fn       create_account;
    avm_has_account_state_fn    has_account_state;
    avm_put_code_fn             put_code;
    avm_get_code_fn             get_code;
    avm_put_storage_fn          put_storage;
    avm_get_storage_fn          get_storage;
    avm_delete_account_fn       delete_account;
    avm_get_balance_fn          get_balance;
    avm_increase_balance_fn     increase_balance;
    avm_decrease_balance_fn     decrease_balance;
    avm_get_nonce_fn            get_nonce;
    avm_increment_nonce_fn      increment_nonce;
    avm_touch_account_fn        touch_account;
    avm_send_signal_fn          send_signal;
    avm_contract_address_fn     contract_address;
    avm_add_log_fn              add_log;
    avm_get_transformed_code_fn get_transformed_code;
    avm_set_transformed_code_fn put_transformed_code;
    avm_get_objectgraph_fn      get_objectgraph;
    avm_set_objectgraph_fn      set_objectgraph;
    avm_get_blockhash_fn        get_blockhash;
    avm_sha256_fn               sha256;
    avm_blake2b_fn              blake2b;
    avm_keccak256_fn            keccak256;
    avm_edveryfy_fn             verify_ed25519;
    avm_remove_storage_fn       remove_storage;
    avm_has_storage_fn          has_storage;
};

typedef struct avm_bytes (*create_contract_fn)(const struct avm_address *address, const uint64_t nonce);
struct avm_rust_utils {
    create_contract_fn new_contract_address;
};

/**
 * Global callback registry.
 */
extern struct avm_callbacks callbacks;

/**
 * Returns whether the byte array is NULL.
 */
extern bool is_null(avm_bytes *bytes);

/**
 * Creates a new byte array, of the given length.
 */
extern struct avm_bytes new_fixed_bytes(u32 length);

/**
 * Creates a NULL byte array.
 */
extern struct avm_bytes new_null_bytes();

/**
 * Releases a byte array.
 */
extern void release_bytes(struct avm_bytes *bytes);

extern jint JNI_OnLoad_avmjni_1();

#if __cplusplus
}
#endif

#endif