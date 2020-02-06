package org.aion.avm.loader;

public class Loader {
    private static final String libraryName = "avmloader";

    // load the native library
    static {
        System.loadLibrary(libraryName);
    }

    public static native void createAccount(long handle, byte[] address);

    public static native boolean hasAccountState(long handle, byte[] address);

    public static native void putCode(long handle, byte[] address, byte[] code);

    public static native byte[] getCode(long handle, byte[] address);

    public static native void putStorage(long handle, byte[] address, byte[] key, byte[] value);

    public static native byte[] getStorage(long handle, byte[] address, byte[] key);

    public static native void deleteAccount(long handle, byte[] address);

    public static native byte[] getBalance(long handle, byte[] address);

    public static native void increaseBalance(long handle, byte[] address, byte[] delta);

    public static native void decreaseBalance(long handle, byte[] address, byte[] delta);

    public static native long getNonce(long handle, byte[] address);

    public static native void incrementNonce(long handle, byte[] address);

    public static native byte[] getTransformedCode(long handle, byte[] address, byte version);

    public static native void setTransformedCode(long handle, byte[] address, byte[] code, byte version);

    public static native byte[] getObjectGraph(long handle, byte[] address);

    public static native void setObjectGraph(long handle, byte[] address, byte[] data);

    /// update substates in kernel
    public static native void touchAccount(long handle, byte[] address, int idx);

    // log contains the encoded data(address+topics+data), idx stands for offset of the substate
    public static native void addLog(long handle, byte[] log, int idx);

    /// helpers to accomplish avm specific tasks
    public static native byte[] sendSignal(long handle, int sig);

    public static native byte[] contract_address(byte[] sender, byte[] nonce);

    public static native byte[] getBlockHashByNumber(long handle, long blockNumber);

    public static native byte[] sha256(byte[] data);

    public static native byte[] blake2b(byte[] data);

    public static native byte[] keccak256(byte[] data);

    public static native boolean edverify(byte[] data, byte[] data1, byte[] data2);

    public static native void removeStorage(long handle, byte[] address, byte[] key);
}
