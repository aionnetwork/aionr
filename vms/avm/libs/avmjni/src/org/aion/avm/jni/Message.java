package org.aion.avm.jni;

import org.aion.vm.api.interfaces.TransactionSideEffects;

import org.aion.vm.api.interfaces.TransactionInterface;
import org.aion.avm.core.BillingRules;

import org.aion.types.AionAddress;
import org.aion.types.Transaction;
// import org.aion.kernel.Transaction;
import org.aion.kernel.SideEffects;

import java.math.BigInteger;
import java.util.Arrays;

/**
 * Represents a transaction context for execution.
 */
public class Message {

    private final byte type;
    private final byte[] address;
    private final byte[] caller;
    private final byte[] origin;
    private final long nonce;
    private final byte[] value;
    private final byte[] data;
    private final long energyLimit;
    private final long energyPrice;
    private byte[] transactionHash;
    private final int basicCost;
    private long transactionTimestamp;
    
    private final byte[] blockPreviousHash;
    private final int internalCallDepth;
    private final SideEffects sideEffects;

    public final long blockTimestamp;
    public final long blockNumber;
    public final long blockEnergyLimit;
    public final byte[] blockCoinbase;
    public final byte[] blockDifficulty;

    byte vm;


    public Message(byte[] bytes) {
        NativeDecoder dec = new NativeDecoder(bytes);

        type = dec.decodeByte();
        address = dec.decodeBytes();
        caller = dec.decodeBytes();
        origin = dec.decodeBytes();
        nonce = dec.decodeLong();
        value = dec.decodeBytes();
        data = dec.decodeBytes();
        energyLimit = dec.decodeLong();
        energyPrice = dec.decodeLong();
        transactionHash = dec.decodeBytes();
        basicCost = dec.decodeInt();
        transactionTimestamp = dec.decodeLong();
        blockTimestamp = dec.decodeLong();
        blockNumber = dec.decodeLong();
        blockEnergyLimit = dec.decodeLong();
        blockCoinbase = dec.decodeBytes();
        blockPreviousHash = dec.decodeBytes();
        blockDifficulty = dec.decodeBytes();
        internalCallDepth = dec.decodeInt();
        sideEffects = new SideEffects();
    }

    public enum Type {
        /**
         * The CREATE is used to deploy a new DApp.
         */
        CREATE(3),
        /**
         * The CALL is used when sending an invocation to an existing DApp.
         */
        CALL(0),
        /**
         * The GARBAGE_COLLECT is a special transaction which asks that the target DApp's storage be deterministically collected.
         * Note that this is the only transaction type which will result in a negative TransactionResult.energyUsed.
         */
        GARBAGE_COLLECT(5);

        private int value;

        Type(int value) {
            this.value = value;
        }

        public int toInt() {
            return this.value;
        }

        public byte toByte() {
            return (byte) this.value;
        }
    }

    public Transaction toAvmTransaction() {
        if (this.isContractCreationTransaction()) {
            return Transaction.contractCreateTransaction(
                new AionAddress(this.caller),
                this.transactionHash,
                BigInteger.valueOf(this.nonce),
                new BigInteger(this.value),
                this.data,
                this.energyLimit,
                this.energyPrice
            );
        } else {
            return Transaction.contractCallTransaction(
                new AionAddress(this.caller),
                new AionAddress(this.address),
                this.transactionHash,
                BigInteger.valueOf(this.nonce),
                new BigInteger(this.value),
                this.data,
                this.energyLimit,
                this.energyPrice
            );
        }
        
    }

    public byte[] getTimestamp() {
        return BigInteger.valueOf(this.transactionTimestamp).toByteArray();
    }

    public byte getKind() {
        if (Constants.DEBUG)
            System.out.printf("message: getKind = %d\n", toAvmType(type).toInt());
        return toAvmType(type).toByte();
    }

    long getTimestampAsLong() {
        return transactionTimestamp;
    }

    /**
     * Returns the {@link org.aion.vm.api.interfaces.VirtualMachine} that this transaction must be
     * executed by in the case of a contract creation.
     *
     * @return The VM to use to create a new contract.
     */
    public byte getTargetVM() {
        return this.vm;
    }

    public AionAddress getContractAddress() {
        throw new AssertionError("Did not expect this to be called.");
    }

    /**
     * Returns the type of transactional logic that this transaction will cause to be executed.
     */
    public Type getType() {
        return toAvmType(type);
    }

    public AionAddress getSenderAddress() {
        return new org.aion.types.AionAddress(caller);
    }

    public AionAddress getDestinationAddress() {
        return new org.aion.types.AionAddress(address);
    }

    public byte[] getNonce() {
        return BigInteger.valueOf(nonce).toByteArray();
    }

    long getNonceAsLong() {
        return nonce;
    }

    public byte[] getValue() {
        return this.value;
    }

    BigInteger getValueAsBigInteger() {
        return new BigInteger(value);
    }

    public byte[] getData() {
        return data;
    }

    public long getEnergyLimit() {
        return energyLimit;
    }

    public long getEnergyPrice() {
        return energyPrice;
    }

    public byte[] getTransactionHash() {
        return transactionHash;
    }

    public long getTransactionCost() {
        if (Constants.DEBUG)
            System.out.println("AVM getTransactionCost");
        return BillingRules.getBasicTransactionCost(getData());
    }

    //Camus: it is strange that vm may change transaction timestamp
    public void setTimestamp(long timestamp) {
        this.transactionTimestamp = timestamp;
    }

    public boolean isContractCreationTransaction() {
        return toAvmType(this.type) == Type.CREATE;
    }

    public String toString() {
        return "TransactionContextHelper{" +
                "type=" + type +
                ", address=" + Arrays.toString(address) +
                ", caller=" + Arrays.toString(caller) +
                ", origin=" + Arrays.toString(origin) +
                ", nonce=" + nonce +
                ", value=" + Arrays.toString(value) +
                ", data=" + Arrays.toString(data) +
                ", energyLimit=" + energyLimit +
                ", energyPrice=" + energyPrice +
                ", transactionHash=" + Arrays.toString(transactionHash) +
                ", basicCost=" + basicCost +
                ", transactionTimestamp=" + transactionTimestamp +
                ", blockTimestamp=" + blockTimestamp +
                ", blockNumber=" + blockNumber +
                ", blockEnergyLimit=" + blockEnergyLimit +
                ", blockCoinbase=" + Arrays.toString(blockCoinbase) +
                ", blockPreviousHash=" + Arrays.toString(blockPreviousHash) +
                ", blockDifficulty=" + Arrays.toString(blockDifficulty) +
                ", internalCallDepth=" + internalCallDepth +
                '}';
    }

    private Type toAvmType(byte type) {
        Type avmType;
        switch (type) {
            case 0x03:
                avmType = Type.CREATE;
                break;
            case 0x05:
                avmType = Type.GARBAGE_COLLECT;
                break;
            default:
                avmType = Type.CALL;
        }

        return avmType;
    }
}
