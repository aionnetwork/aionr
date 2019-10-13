package org.aion.avm.jni;

import org.aion.types.AionAddress;
import org.aion.avm.core.IExternalState;
import org.aion.avm.core.NodeEnvironment;
import org.aion.avm.loader.Loader;

import java.math.BigInteger;
import java.util.Set;

/**
 * JNI binding of the kernel interface.
 */
public class NativeKernelInterface implements IExternalState {

    // private static final String libraryName = "avmjni";
    private boolean isLocalCall;
    private static final long CONTRACT_CREATE_TX_NRG_MIN = 200000;
    private static final long CONTRACT_CREATE_TX_NRG_MAX = 5000000;
    private static final long TX_NRG_MIN = 21000;
    private static final long TX_NRG_MAX = 2000000;

    // private final Loader loader;

    public static boolean isValidNrgContractCreate(long nrg) {
        return nrg >= CONTRACT_CREATE_TX_NRG_MIN && nrg <= CONTRACT_CREATE_TX_NRG_MAX;
    }

    public static boolean isValidNrgTx(long nrg) {
        return nrg >= TX_NRG_MIN && nrg <= TX_NRG_MAX;
    }

    // load the native library
    // static {
    //     System.loadLibrary(libraryName);
    // }

    // store the pointer of a native KernelInterface object.
    private long handle;

    public NativeKernelInterface(long handle, boolean isLocal) {
        this.handle = handle;
        this.isLocalCall = isLocal;
    }

    @Override
    public NativeKernelInterface newChildExternalState() {
        return new NativeKernelInterface(this.handle, this.isLocalCall);
    }

    public void touchAccount(byte[] addr, int index_of_substate) {
        Loader.touchAccount(handle, addr, index_of_substate);
    }

    public void addLog(byte[] log, int idx) {
        Loader.addLog(handle, log, idx);
    }

    public byte[] sendSignal(int sig_num) {
        return Loader.sendSignal(handle, sig_num);
    }

    @Override
    public void createAccount(AionAddress address) {
        Loader.createAccount(handle, address.toByteArray());
    }

    @Override
    public boolean hasAccountState(AionAddress address) {
        return Loader.hasAccountState(handle, address.toByteArray());
    }

    @Override
    public void putCode(AionAddress address, byte[] code) {
        Loader.putCode(handle, address.toByteArray(), code);
    }

    @Override
    public byte[] getCode(AionAddress address) {
        return Loader.getCode(handle, address.toByteArray());
    }

    @Override
    public void putStorage(AionAddress address, byte[] key, byte[] value) {
        Loader.putStorage(handle, address.toByteArray(), key, value);
    }

    @Override
    public byte[] getStorage(AionAddress address, byte[] key) {
        return Loader.getStorage(handle, address.toByteArray(), key);
    }

    @Override
    public void deleteAccount(AionAddress address) {
        if (!this.isLocalCall) {
            Loader.deleteAccount(handle, address.toByteArray());
        }
    }

    @Override
    public boolean accountNonceEquals(AionAddress address, BigInteger nonce) {
        return this.isLocalCall || nonce.compareTo(this.getNonce(address)) == 0;
    }

    @Override
    public BigInteger getBalance(AionAddress address) {
        byte[] balance = Loader.getBalance(handle, address.toByteArray());
        return new BigInteger(1, balance);
    }

    @Override
    public void adjustBalance(AionAddress address, BigInteger delta) {
        // System.out.println(String.format("Native: avm adjust balance: %d", delta.longValue()));
        if (delta.signum() > 0) {
            Loader.increaseBalance(handle, address.toByteArray(), delta.toByteArray());
        } else if (delta.signum() < 0) {
            Loader.decreaseBalance(handle, address.toByteArray(), delta.negate().toByteArray());
        }
    }

    @Override
    public BigInteger getNonce(AionAddress address) {
        return BigInteger.valueOf(Loader.getNonce(handle, address.toByteArray()));
    }

    @Override
    public void incrementNonce(AionAddress address) {
        Loader.incrementNonce(handle, address.toByteArray());
    }

    @Override
    public boolean accountBalanceIsAtLeast(AionAddress address, BigInteger amount) {
        return this.isLocalCall || getBalance(address).compareTo(amount) >= 0;
    }
    
    @Override
    public boolean isValidEnergyLimitForNonCreate(long energyLimit) {
      return this.isLocalCall || isValidNrgTx(energyLimit);
    }

    @Override
    public boolean isValidEnergyLimitForCreate(long energyLimit) {
      return (this.isLocalCall) || isValidNrgContractCreate(energyLimit);
    }

    @Override
    public boolean destinationAddressIsSafeForThisVM(AionAddress address) {
        byte[] code = getCode(address);
        return (code == null) || (code.length == 0) || !(code[0] == 0x60 && code[1] == 0x50);
    }

    @Override
    public byte[] getBlockHashByNumber(long blockNumber) {
        return Loader.getBlockHashByNumber(handle, blockNumber);
    }

    @Override
    public void refundAccount(AionAddress address, BigInteger amount) {
        // This method may have special logic in the kernel. Here it is just adjustBalance.
        adjustBalance(address, amount);
    }

    @Override
    public void removeStorage(AionAddress address, byte[] key) {
        Loader.removeStorage(handle, address.toByteArray(), key);
    }

    @Override
    public byte[] getObjectGraph(AionAddress a) {
        return Loader.getObjectGraph(handle, a.toByteArray());
    }

    @Override
    public void putObjectGraph(AionAddress a, byte[] data) {
        Loader.setObjectGraph(handle, a.toByteArray(), data);
    }

    // Camus: this should not be in kernel interface
    @Override
    public AionAddress getMinerAddress() {
        throw new AssertionError("Did not expect this to be called.");
    }

    // Camus: this should not be in kernel interface
    @Override
    public BigInteger getBlockDifficulty() {
        throw new AssertionError("Did not expect this to be called.");
    }

    // Camus: this should not be in kernel interface
    @Override
    public long getBlockEnergyLimit() {
        throw new AssertionError("Did not expect this to be called.");
    }

    // Camus: this should not be in kernel interface
    @Override
    public long getBlockTimestamp() {
        throw new AssertionError("Did not expect this to be called.");
    }

    // Camus: this should not be in kernel interface
    @Override
    public long getBlockNumber() {
        throw new AssertionError("Did not expect this to be called.");
    }

    // @Override
    // public Set<byte[]> getTouchedAccounts() {
    //     throw new AssertionError("This class does not implement this method.");
    // }

    @Override
    public void commitTo(IExternalState target) { }

    @Override
    public void commit() { }

    @Override
    public void setTransformedCode(AionAddress address, byte[] code) {
        Loader.setTransformedCode(handle, address.toByteArray(), code);

    }

    @Override
    public byte[] getTransformedCode(AionAddress address) {
        return Loader.getTransformedCode(handle, address.toByteArray());
    }

    // public static native void createAccount(long handle, byte[] address);

    // public static native boolean hasAccountState(long handle, byte[] address);

    // public static native void putCode(long handle, byte[] address, byte[] code);

    // public static native byte[] getCode(long handle, byte[] address);

    // public static native void putStorage(long handle, byte[] address, byte[] key, byte[] value);

    // public static native byte[] getStorage(long handle, byte[] address, byte[] key);

    // public static native void deleteAccount(long handle, byte[] address);

    // public static native byte[] getBalance(long handle, byte[] address);

    // public static native void increaseBalance(long handle, byte[] address, byte[] delta);

    // public static native void decreaseBalance(long handle, byte[] address, byte[] delta);

    // public static native long getNonce(long handle, byte[] address);

    // public static native void incrementNonce(long handle, byte[] address);

    // public static native byte[] getTransformedCode(long handle, byte[] address);

    // public static native void setTransformedCode(long handle, byte[] address, byte[] code);

    // public static native byte[] getObjectGraph(long handle, byte[] address);

    // public static native void setObjectGraph(long handle, byte[] address, byte[] data);

    // /// update substates in kernel
    // public static native void touchAccount(long handle, byte[] address, int idx);

    // // log contains the encoded data(address+topics+data), idx stands for offset of the substate
    // public static native void addLog(long handle, byte[] log, int idx);

    // /// helpers to accomplish avm specific tasks
    // public static native byte[] sendSignal(long handle, int sig);

    public static byte[] contract_address(byte[] sender, byte[] nonce) {
        return Loader.contract_address(sender, nonce);
    }

    // public static native byte[] getBlockHashByNumber(long handle, long blockNumber);

    // public static native byte[] sha256(byte[] data);

    // public static native byte[] blake2b(byte[] data);

    // public static native byte[] keccak256(byte[] data);

    // public static native boolean edverify(byte[] data, byte[] data1, byte[] data2);

    // public static native void removeStorage(long handle, byte[] address, byte[] key);
}
