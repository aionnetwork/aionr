package org.aion.avm.jni;

import org.aion.avm.core.IExternalState;
import org.aion.types.AionAddress;

import java.util.List;
import java.util.Set;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.function.Consumer;
import java.math.BigInteger;
import java.util.HashMap;
import java.util.HashSet;

public class Substate implements IExternalState {
    final private IExternalState parent;
    private final List<Consumer<IExternalState>> writeLog;
    /// cached nonces
    private final HashMap<AionAddress, BigInteger> nonces;
    /// cached balances
    private final HashMap<AionAddress, BigInteger> balances;
    /// cached object graph
    private final HashMap<AionAddress, byte[]> objectGraphs;
    /// storage keys and values
    private final HashMap<AionAddress, HashMap<String, byte[]>> storage;
    // private final HashMap<byte[], byte[]> values;
    
    /// block info (act as env info)
    private EnvInfo info;
    private boolean isLocalCall;

    private static final String ADDR_OWNER =
            "0000000000000000000000000000000000000000000000000000000000000000";
    private static final String ADDR_TOTAL_CURRENCY =
            "0000000000000000000000000000000000000000000000000000000000000100";

    private static final String ADDR_TOKEN_BRIDGE =
            "0000000000000000000000000000000000000000000000000000000000000200";
    private static final String ADDR_TOKEN_BRIDGE_INITIAL_OWNER =
            "a008d7b29e8d1f4bfab428adce89dc219c4714b2c6bf3fd1131b688f9ad804aa";

    private static final String ADDR_ED_VERIFY =
            "0000000000000000000000000000000000000000000000000000000000000010";
    private static final String ADDR_BLAKE2B_HASH =
            "0000000000000000000000000000000000000000000000000000000000000011";
    private static final String ADDR_TX_HASH =
            "0000000000000000000000000000000000000000000000000000000000000012";

    private class EnvInfo {
        private AionAddress coinbase;
        private long blockTimestamp;
        private BigInteger blockDifficulty;
        private long blockGasLimit;
        private long blockNumber;
    }

    private static final char[] HEX_ARRAY = "0123456789ABCDEF".toCharArray();
    public static String bytesToHex(byte[] bytes) {
        char[] hexChars = new char[bytes.length * 2];
        for (int j = 0; j < bytes.length; j++) {
            int v = bytes[j] & 0xFF;
            hexChars[j * 2] = HEX_ARRAY[v >>> 4];
            hexChars[j * 2 + 1] = HEX_ARRAY[v & 0x0F];
        }
        return new String(hexChars);
    }

    public Substate(IExternalState parent, boolean isLocal) {
        this.parent = parent;
        this.writeLog = new ArrayList<>();
        this.nonces = new HashMap<>();
        this.balances = new HashMap<>();
        this.objectGraphs = new HashMap<>();
        this.storage = new HashMap<>();
        // this.values = new HashMap<>();
        this.info = new EnvInfo();
        this.isLocalCall = isLocal;
    }

    @Override
    public Substate newChildExternalState() {
        return new Substate(this, this.isLocalCall);
    }

    // block info is regarded as EnvInfo for transactions
    public void updateEnvInfo(Message msg) {
        byte[] difficulty = Arrays.copyOfRange(msg.blockDifficulty, 8, 16);
        // NativeDecoder decoder = new NativeDecoder(difficulty);
        // this.info.blockDifficulty = decoder.decodeLong();
        this.info.blockDifficulty = new BigInteger(difficulty);
        this.info.blockTimestamp = msg.blockTimestamp;
        this.info.blockGasLimit = msg.blockEnergyLimit;
        this.info.blockNumber = msg.blockNumber;
        this.info.coinbase = new AionAddress(msg.blockCoinbase);
    }

    private boolean isPrecompiledContract(AionAddress address) {
        switch (address.toString()) {
            case ADDR_TOKEN_BRIDGE:
            case ADDR_ED_VERIFY:
            case ADDR_BLAKE2B_HASH:
            case ADDR_TX_HASH:
                return true;
            case ADDR_TOTAL_CURRENCY:
            default:
                return false;
        }
    }

    @Override
    public void createAccount(AionAddress address) {
        if (Constants.DEBUG) {
            System.out.printf("JNI: create account: %s", address);
        }
        Consumer<IExternalState> write = (kernel) -> {
            kernel.createAccount(address);
        };
        writeLog.add(write);
    }

    @Override
    public boolean hasAccountState(AionAddress address) {
        if (Constants.DEBUG) {
            System.out.printf("JNI: check account state: %s", address);
        }
        return this.parent.hasAccountState(address);
    }

    @Override
    public void putCode(AionAddress address, byte[] code) {
        if (Constants.DEBUG) {
            System.out.printf("JNI: save code: %s", address);
        }
        Consumer<IExternalState> write = (kernel) -> {
            kernel.putCode(address, code);
        };
        writeLog.add(write);
    }

    @Override
    public byte[] getCode(AionAddress address) {
        if (Constants.DEBUG) {
            System.out.printf("JNI: get code of %s", address);
        }
        return this.parent.getCode(address);
    }

    @Override
    public void putStorage(AionAddress address, byte[] _key, byte[] value) {
        String key = bytesToHex(_key);
        Consumer<IExternalState> write = (kernel) -> {
            kernel.putStorage(address, _key, value);
        };
        writeLog.add(write);


        HashMap<String, byte[]> account_storage = this.storage.get(address);
        if (account_storage == null) {
            this.storage.put(address, new HashMap<>());
            this.storage.get(address).put(key, value);
        } else {
            // key set is not null but the key is not found
            account_storage.put(key, value);
        }
    }

    @Override
    public byte[] getStorage(AionAddress address, byte[] _key) {
        String key = bytesToHex(_key);
        byte[] value;
        HashMap<String, byte[]> account_storage = this.storage.get(address);
        if (null == account_storage) {
            value = this.parent.getStorage(address, _key);
            // this.keys.put(address, new HashSet<>());
            // this.keys.get(address).add(key);
            // this.values.put(key, value);
            this.storage.put(address, new HashMap<>());
            this.storage.get(address).put(key, value);
        } else {
            // has local keys
            if (account_storage.containsKey(key)) {
                value = account_storage.get(key);
            } else {
                value = this.parent.getStorage(address, _key);
                // key/value always update together
                // localKeys.add(key);
                // this.values.put(key, value);
                account_storage.put(key, value);
            }
        }

        return value;
    }

    @Override
    public void deleteAccount(AionAddress address) {
        Consumer<IExternalState> write = (kernel) -> {
            kernel.deleteAccount(address);
        };
        writeLog.add(write);
    }

    @Override
    public boolean accountNonceEquals(AionAddress address, BigInteger nonce) {
        if (Constants.DEBUG) {
            System.out.print("current Nonce = ");
            System.out.println(getNonce(address));
        }
        
        return this.isLocalCall || getNonce(address).compareTo(nonce) == 0;
    }

    @Override
    public BigInteger getBalance(AionAddress address) {
        if (Constants.DEBUG) {
            System.out.printf("JNI: getBalance of ");
            System.out.println(address);
        }
        BigInteger balance = this.balances.get(address);
        if (null == balance) {
            balance = this.parent.getBalance(address);
            this.balances.put(address, balance);
        }
        return balance;
    }

    @Override
    public void adjustBalance(AionAddress address, BigInteger delta) {
        if (Constants.DEBUG) {
            System.out.printf("try adjust balance: %d\n", delta.longValue());
        }
        Consumer<IExternalState> write = (kernel) -> {
            kernel.adjustBalance(address, delta);
        };
        writeLog.add(write);

        this.balances.put(address, getBalance(address).add(delta));
    }

    @Override
    public BigInteger getNonce(AionAddress address) {
        if (Constants.DEBUG) {
            System.out.print("JNI: try getNonce of: ");
            System.out.println(address);
        }
        
        BigInteger nonce = this.nonces.get(address);
        if (nonce == null) {
            nonce = this.parent.getNonce(address);
            this.nonces.put(address, nonce);
        }
        return nonce;
    }

    @Override
    public void incrementNonce(AionAddress address) {
        Consumer<IExternalState> write = (kernel) -> {
            kernel.incrementNonce(address);
        };
        writeLog.add(write);
        BigInteger nonce = this.nonces.get(address);
        if (nonce == null) {
            nonce = this.parent.getNonce(address);
        }
        this.nonces.put(address, nonce.add(BigInteger.ONE));
    }

    @Override
    public boolean accountBalanceIsAtLeast(AionAddress address, BigInteger amount) {
        return this.isLocalCall || getBalance(address).compareTo(amount) >= 0;
    }
    
    @Override
    public boolean isValidEnergyLimitForNonCreate(long energyLimit) {
        return this.parent.isValidEnergyLimitForNonCreate(energyLimit);
    }

    @Override
    public boolean isValidEnergyLimitForCreate(long energyLimit) {
        return this.parent.isValidEnergyLimitForCreate(energyLimit);
    }

    @Override
    public boolean destinationAddressIsSafeForThisVM(AionAddress address) {
        if (isPrecompiledContract(address)) {
            return false;
        }
        return this.parent.destinationAddressIsSafeForThisVM(address);
    }

    @Override
    public byte[] getBlockHashByNumber(long blockNumber) {
        return this.parent.getBlockHashByNumber(blockNumber);
    }

    @Override
    public void refundAccount(AionAddress address, BigInteger amount) {
        adjustBalance(address, amount);
    }

    @Override
    public void removeStorage(AionAddress address, byte[] key) {
        putStorage(address, key, null);
        Consumer<IExternalState> write = (kernel) -> {
            kernel.removeStorage(address, key);
        };
        writeLog.add(write);
    }

    @Override
    public byte[] getObjectGraph(AionAddress a) {
        if (this.objectGraphs.get(a) == null) {
            if (Constants.DEBUG) {
                System.out.println("JNI: try updating object graph");
            }
            byte[] graph = parent.getObjectGraph(a);
            this.objectGraphs.put(a, graph);
            return graph;
        }

        return this.objectGraphs.get(a);
    }

    @Override
    public void putObjectGraph(AionAddress a, byte[] data) {
        if (Constants.DEBUG) {
            System.out.printf("JNI: save object graph at ");
            System.out.println(a);
        }
        this.objectGraphs.put(a, data);
        Consumer<IExternalState> write = (kernel) -> {
            kernel.putObjectGraph(a, data);
        };
        writeLog.add(write);

    }

    @Override
    public void commitTo(IExternalState target) { }

    @Override
    public void commit() {
        for (Consumer<IExternalState> mutation : this.writeLog) {
            mutation.accept(this.parent);
        }
    }

    @Override
    public AionAddress getMinerAddress() {
        if (Constants.DEBUG) {
            System.out.printf("JNI: try to get miner address\n");
        }
        
        return this.info.coinbase;
    }

    @Override
    public BigInteger getBlockDifficulty() {
        if (Constants.DEBUG) {
            System.out.print("Block Difficulty: ");
            System.out.println(this.info.blockDifficulty);
        }
        
        return this.info.blockDifficulty;
    }

    @Override
    public long getBlockEnergyLimit() {
        return this.info.blockGasLimit;
    }

    @Override
    public long getBlockTimestamp() {
        return this.info.blockTimestamp;
    }

    @Override
    public long getBlockNumber() {
        return this.info.blockNumber;
    }

    @Override
    public void setTransformedCode(AionAddress address, byte[] bytes) {
        Consumer<IExternalState> write = (kernel) -> {
            kernel.setTransformedCode(address, bytes);
        };
        writeLog.add(write);
    }

    @Override
    public byte[] getTransformedCode(AionAddress address) {
        return parent.getTransformedCode(address);
    }

    @Override
    public boolean hasStorage(AionAddress address) {
        return parent.hasStorage(address);
    }
}