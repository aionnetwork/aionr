package org.aion;

import org.aion.avm.userlib.abi.ABIEncoder;
import org.aion.avm.tooling.abi.Callable;

import avm.Blockchain;
import avm.Address;
import avm.Result;

public class UserAPI {

	@Callable
    public static Address getAddress() {
        return Blockchain.getAddress();
    }

	@Callable
    public static Address getCaller() {
        return Blockchain.getCaller();
    }

	@Callable
    public static Address getOrigin() {
        return Blockchain.getOrigin();
    }

	@Callable
    public static long getEnergyLimit() {
        return Blockchain.getEnergyLimit();
    }

	@Callable
    public static long getEnergyPrice() {
        return Blockchain.getEnergyPrice();
    }

	@Callable
    public static long getValue() {
        return Blockchain.getValue().longValue();
    }

	@Callable
    public static byte[] getData() {
        return Blockchain.getData();
    }

	@Callable
    public static long getBlockTimestamp() {
        return Blockchain.getBlockTimestamp();
    }

	@Callable
    public static long getBlockNumber() {
        return Blockchain.getBlockNumber();
    }

	@Callable
    public static long getBlockEnergyLimit() {
        return Blockchain.getBlockEnergyLimit();
    }

	@Callable
    public static Address getBlockCoinbase() {
        return Blockchain.getBlockCoinbase();
    }

	@Callable
    public static long getBlockDifficulty() {
        return Blockchain.getBlockDifficulty().longValue();
    }

	@Callable
    public static long getBalance(Address address) {
        return Blockchain.getBalance(address).longValue();
    }

	@Callable
    public static long getBalanceOfThisContract() {
        return Blockchain.getBalanceOfThisContract().longValue();
    }

	@Callable
    public static int getCodeSize(Address address) {
        return Blockchain.getCodeSize(address);
    }

	@Callable
    public static long getRemainingEnergy() {
        return Blockchain.getRemainingEnergy();
    }

	@Callable
    // TODO: need another AVM contract
    public static void call(Address targetAddress, int value, byte[] data, long energylimit) {
        byte[] returnData = {0, 1, 2, 3};
//        return new Result(true, returnData);
    }

	@Callable
    public static void create(int value, byte[] data, long energyLimit) {
        byte[] returnData = {0, 1, 2, 3};
//        return new Result(true, returnData);
    }

	@Callable
    public static void selfDestruct(Address beneficiary) {
        Blockchain.selfDestruct(beneficiary);
    }

    @Callable
    public static void log_on_failure() {
        byte[] data = new byte[]{0x00, 0x01, 0x02};
        Blockchain.log(data);
        Blockchain.invalid();
    }

	@Callable
    public static void log1() {
        byte[] data = new byte[]{0x00, 0x01, 0x02};
        Blockchain.log(data);
    }

	@Callable
    public static void log2() {
        byte[] topic1 = new byte[]{0x00, 0x01, 0x02};
        byte[] topic2 = new byte[]{0x05, 0x06, 0x07};
        byte[] data = new byte[]{0x0a, 0x0a, 0x0a};
        Blockchain.log(topic1, topic2, data);
    }

	@Callable
    public static void log3() {
        byte[] topic1 = new byte[]{0x00, 0x01, 0x02};
        byte[] topic2 = new byte[]{0x05, 0x06, 0x07};
        byte[] topic3 = new byte[]{0x0a, 0x0a};
        byte[] data = new byte[]{0x0b, 0x0b, 0x0b};
        Blockchain.log(topic1, topic2, topic3, data);
    }

	@Callable
    public static void log4() {
        byte[] topic1 = new byte[]{0x00, 0x01, 0x02};
        byte[] topic2 = new byte[]{0x05, 0x06, 0x07};
        byte[] topic3 = new byte[]{0x0a, 0x0a};
        byte[] topic4 = new byte[]{0x0b, 0x0b};
        byte[] data = new byte[]{0x0c, 0x0c, 0x0c};
        Blockchain.log(topic1, topic2, topic3, topic4, data);
    }

	@Callable
    public static byte[] blake2b() {
        byte[] data = "hello, blake2b".getBytes();
        return Blockchain.blake2b(data);
    }

	@Callable
    public static byte[] sha256() {
        byte[] data = "hello, sha256".getBytes();
        return Blockchain.sha256(data);
    }

	@Callable
    public static byte[] keccak256() {
        byte[] data = "hello, keccak256".getBytes();
        return Blockchain.keccak256(data);
    }

	@Callable
    public static void revert() {
        Blockchain.revert();
    }

	@Callable
    public static void invalid() {
        Blockchain.invalid();
    }

	@Callable
    public static void require(boolean condition) {
        Blockchain.require(condition);
    }

	@Callable
    public static void print(java.lang.String message) {
        Blockchain.print(message);
    }

	@Callable
    public static void println(java.lang.String message) {
        Blockchain.println(message);
    }

	@Callable
    public static boolean edVerify(byte[] data, byte[] signature, byte[] publicKey) {
        return Blockchain.edVerify(data, signature, publicKey);
    }
}
