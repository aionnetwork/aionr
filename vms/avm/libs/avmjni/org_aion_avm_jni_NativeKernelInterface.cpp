#include "org_aion_avm_jni_NativeKernelInterface.h"
#include "avm.h"

//#include <cstring>
//#include <iostream>

#define ADDRESS_LENGTH sizeof(avm_address)
#define VALUE_LENGTH   sizeof(avm_value)

using namespace std;

/**
 * Global callback registry.
 */
struct avm_callbacks callbacks;

jint JNI_OnLoad_avmjni_1() {
  return JNI_VERSION_10;
}

/**
 * Returns whether the byte array is NULL.
 */
bool is_null(avm_bytes *bytes) {
    return bytes->pointer == NULL;
}

/**
 * Creates a new byte array, of the given size.
 */
struct avm_bytes new_fixed_bytes(u32 length) {
    struct avm_bytes bytes;
    bytes.length = length;
    bytes.pointer = (u8 *)malloc(length);
    return bytes;
}

/**
 * Creates a NULL byte array.
 */
struct avm_bytes new_null_bytes() {
    struct avm_bytes bytes;
    bytes.length = 0;
    bytes.pointer = NULL;
    return bytes;
}

/**
 * Releases a byte array.
 */
void release_bytes(struct avm_bytes *bytes) {
    if (bytes->pointer) {
        free(bytes->pointer);

        bytes->length = 0;
        bytes->pointer = NULL;
    }
}

/**
 * Converts an address from the JVM heap to the native counterpart.
 */
static struct avm_address load_address(JNIEnv *env, jbyteArray address)
{
    struct avm_address ret;
    env->GetByteArrayRegion(address, 0, ADDRESS_LENGTH, (jbyte *)ret.bytes);
    return ret;
}

/**
 * Converts a value from the JVM heap to the native counterpart.
 */
static struct avm_value load_value(JNIEnv *env, jbyteArray value)
{
    struct avm_value ret;
    unsigned length = (unsigned) env->GetArrayLength(value);
    if (length <= VALUE_LENGTH) {
        // big-endian
        memset(ret.bytes, 0, VALUE_LENGTH);
        env->GetByteArrayRegion(value, 0, length, (jbyte *)ret.bytes + (VALUE_LENGTH - length));
    } else {
        env->GetByteArrayRegion(value, length - VALUE_LENGTH, VALUE_LENGTH, (jbyte *)ret.bytes);
    }
    return ret;
}

/**
 * Copies a byte array from the JVM heap to the native counterpart.
 */
static struct avm_bytes load_bytes(JNIEnv *env, jbyteArray bytes) {
    if (bytes == NULL) {
        return new_null_bytes();
    } else {
        // allocate the required memory
        avm_bytes ret = new_fixed_bytes((u32) env->GetArrayLength(bytes));
        // copy the data
        env->GetByteArrayRegion(bytes, 0, ret.length, (jbyte *)ret.pointer);
        return ret;
    }
}

/**
 * Creates a byte array in the JVM and initialize it with the given data.
 */
static jbyteArray to_jbyteArray(JNIEnv *env, u8 *ptr, u32 size) {
    jbyteArray bytes = env->NewByteArray(size);
    env->SetByteArrayRegion(bytes, 0, size, (jbyte *)ptr);

    return bytes;
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    createAccount
 * Signature: (J[B)V
 */
JNIEXPORT void JNICALL Java_org_aion_avm_jni_NativeKernelInterface_createAccount
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray address)
{
    struct avm_address a = load_address(env, address);

    callbacks.create_account((void *)handle, &a);
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    hasAccountState
 * Signature: (J[B)Z
 */
JNIEXPORT jboolean JNICALL Java_org_aion_avm_jni_NativeKernelInterface_hasAccountState
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray address)
{
    struct avm_address a = load_address(env, address);

    u32 ret = callbacks.has_account_state((void *)handle, &a);

    return ret == 0 ? JNI_FALSE : JNI_TRUE;
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    putCode
 * Signature: (J[B[B)V
 */
JNIEXPORT void JNICALL Java_org_aion_avm_jni_NativeKernelInterface_putCode
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray address, jbyteArray code)
{
    struct avm_address a = load_address(env, address);
    struct avm_bytes c = load_bytes(env, code);

    callbacks.put_code((void *)handle, &a, &c);

    // release the buffer
    release_bytes(&c);
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    getCode
 * Signature: (J[B)[B
 */
JNIEXPORT jbyteArray JNICALL Java_org_aion_avm_jni_NativeKernelInterface_getCode
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray address)
{
    struct avm_address a = load_address(env, address);

    // ask the client for account code
    struct avm_bytes c = callbacks.get_code((void *)handle, &a);

    // convert into JVM byte array.
    jbyteArray ret = is_null(&c) ? NULL : to_jbyteArray(env, c.pointer, c.length);

    // release the buffer
    release_bytes(&c);

    return ret;
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    putStorage
 * Signature: (J[B[B[B)V
 */
JNIEXPORT void JNICALL Java_org_aion_avm_jni_NativeKernelInterface_putStorage
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray address, jbyteArray key, jbyteArray value)
{
    struct avm_address a = load_address(env, address);
    struct avm_bytes k = load_bytes(env, key);
    struct avm_bytes v = load_bytes(env, value);

    callbacks.put_storage((void *)handle, &a, &k, &v);

    // release the buffer
    release_bytes(&k);
    release_bytes(&v);
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    getStorage
 * Signature: (J[B[B)[B
 */
JNIEXPORT jbyteArray JNICALL Java_org_aion_avm_jni_NativeKernelInterface_getStorage
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray address, jbyteArray key)
{
    struct avm_address a = load_address(env, address);
    struct avm_bytes k = load_bytes(env, key);

    // ask the client for storage value
    struct avm_bytes v = callbacks.get_storage((void *)handle, &a, &k);

    // convert into JVM byte array.
    jbyteArray ret = is_null(&v) ? NULL : to_jbyteArray(env, v.pointer, v.length);

    // release the buffer
    release_bytes(&k);
    release_bytes(&v);

    return ret;
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    deleteAccount
 * Signature: (J[B)V
 */
JNIEXPORT void JNICALL Java_org_aion_avm_jni_NativeKernelInterface_deleteAccount
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray address)
{
    struct avm_address a = load_address(env, address);

    callbacks.delete_account((void *)handle, &a);
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    getBalance
 * Signature: (J[B)[B
 */
JNIEXPORT jbyteArray JNICALL Java_org_aion_avm_jni_NativeKernelInterface_getBalance
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray address)
{
    struct avm_address a = load_address(env, address);

    struct avm_value v = callbacks.get_balance((void *)handle, &a);

    return to_jbyteArray(env, v.bytes, VALUE_LENGTH);
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    increaseBalance
 * Signature: (J[B[B)V
 */
JNIEXPORT void JNICALL Java_org_aion_avm_jni_NativeKernelInterface_increaseBalance
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray address, jbyteArray value)
{
    struct avm_address a = load_address(env, address);
    struct avm_value v = load_value(env, value);

    callbacks.increase_balance((void *)handle, &a, &v);
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    decreaseBalance
 * Signature: (J[B[B)V
 */
JNIEXPORT void JNICALL Java_org_aion_avm_jni_NativeKernelInterface_decreaseBalance
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray address, jbyteArray value)
{
    struct avm_address a = load_address(env, address);
    struct avm_value v = load_value(env, value);

    callbacks.decrease_balance((void *)handle, &a, &v);
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    getNonce
 * Signature: (J[B)J
 */
JNIEXPORT jlong JNICALL Java_org_aion_avm_jni_NativeKernelInterface_getNonce
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray address)
{
    struct avm_address a = load_address(env, address);

    return callbacks.get_nonce((void *)handle, &a);
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    incrementNonce
 * Signature: (J[B)V
 */
JNIEXPORT void JNICALL Java_org_aion_avm_jni_NativeKernelInterface_incrementNonce
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray address)
{
    struct avm_address a = load_address(env, address);

    callbacks.increment_nonce((void *)handle, &a);
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    touchAccount
 * Signature: (J[BI)V
 */
JNIEXPORT void JNICALL Java_org_aion_avm_jni_NativeKernelInterface_touchAccount
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray address, jint substate_index)
{
    struct avm_address a = load_address(env, address);

    callbacks.touch_account((void *)handle, &a, substate_index);
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    sendSignal
 * Signature: (JI)[B
 */
JNIEXPORT jbyteArray JNICALL Java_org_aion_avm_jni_NativeKernelInterface_sendSignal
  (JNIEnv *env, jclass clazz, jlong handle, jint signal_num)
{
    // ask the client for storage value
    struct avm_bytes v = callbacks.send_signal((void *)handle, signal_num);

    // convert into JVM byte array.
    jbyteArray ret = is_null(&v) ? NULL : to_jbyteArray(env, v.pointer, v.length);

    // release the buffer
    release_bytes(&v);

    return ret;
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    contract_address
 * Signature: ([B[B)[B
 */
JNIEXPORT jbyteArray JNICALL Java_org_aion_avm_jni_NativeKernelInterface_contract_1address
  (JNIEnv *env, jclass clazz, jbyteArray sender, jbyteArray nonce)
{
  struct avm_address a = load_address(env, sender);
  struct avm_bytes n = load_bytes(env, nonce);

  struct avm_bytes v = callbacks.contract_address(&a, &n);

  jbyteArray ret = is_null(&v)? NULL:to_jbyteArray(env, v.pointer, v.length);

  release_bytes(&v);

  return ret;
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    addLog
 * Signature: (J[BI)V
 */
JNIEXPORT void JNICALL Java_org_aion_avm_jni_NativeKernelInterface_addLog
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray avmLog, jint index)
{
  struct avm_bytes n = load_bytes(env, avmLog);
  callbacks.add_log((void *)handle, &n, index);
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    getTransformedCode
 * Signature: (J[B)[B
 */
JNIEXPORT jbyteArray JNICALL Java_org_aion_avm_jni_NativeKernelInterface_getTransformedCode
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray address)
{
    struct avm_address a = load_address(env, address);

    // ask the client for account code
    struct avm_bytes c = callbacks.get_transformed_code((void *)handle, &a);

    // convert into JVM byte array.
    jbyteArray ret = is_null(&c) ? NULL : to_jbyteArray(env, c.pointer, c.length);

    // release the buffer
    release_bytes(&c);

    return ret;
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    setTransformedCode
 * Signature: (J[B[B)V
 */
JNIEXPORT void JNICALL Java_org_aion_avm_jni_NativeKernelInterface_setTransformedCode
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray address, jbyteArray code)
{
    struct avm_address a = load_address(env, address);
    struct avm_bytes c = load_bytes(env, code);

    callbacks.put_transformed_code((void *)handle, &a, &c);

    // release the buffer
    release_bytes(&c);
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    getObjectGraph
 * Signature: (J[B)[B
 */
JNIEXPORT jbyteArray JNICALL Java_org_aion_avm_jni_NativeKernelInterface_getObjectGraph
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray address)
{
    struct avm_address a = load_address(env, address);
    // ask the client for account code
    struct avm_bytes c = callbacks.get_objectgraph((void *)handle, &a);

    // convert into JVM byte array.
    jbyteArray ret = is_null(&c) ? NULL : to_jbyteArray(env, c.pointer, c.length);

    // release the buffer
    release_bytes(&c);

    return ret;
}

/*
 * Class:     org_aion_avm_jni_NativeKernelInterface
 * Method:    setObjectGraph
 * Signature: (J[B[B)V
 */
JNIEXPORT void JNICALL Java_org_aion_avm_jni_NativeKernelInterface_setObjectGraph
  (JNIEnv *env, jclass clazz, jlong handle, jbyteArray address, jbyteArray data)
{
    struct avm_address a = load_address(env, address);
    struct avm_bytes c = load_bytes(env, data);

    callbacks.set_objectgraph((void *)handle, &a, &c);

    // release the buffer
    release_bytes(&c);
}