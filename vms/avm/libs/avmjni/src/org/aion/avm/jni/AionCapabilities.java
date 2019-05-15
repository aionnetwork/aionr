package org.aion.avm.jni;

import org.aion.avm.core.IExternalCapabilities;
import org.aion.types.Address;
import org.aion.vm.api.interfaces.TransactionContext;
import org.aion.vm.api.interfaces.TransactionInterface;

public class AionCapabilities implements IExternalCapabilities {

    @Override
    public byte[] sha256(byte[] bytes) {
        throw new RuntimeException("sha256 not supported");
    }

    @Override
    public byte[] blake2b(byte[] bytes) {
        throw new RuntimeException("blake2b not supported");
    }

    @Override
    public byte[] keccak256(byte[] bytes) {
        throw new RuntimeException("keccak256 not supported");
    }

    @Override
    public boolean verifyEdDSA(byte[] bytes, byte[] bytes1, byte[] bytes2) {
        throw new RuntimeException("verifyEsDSA not supported");
    }

    @Override
    public Address generateContractAddress(TransactionInterface txMessage) {
        byte[] sender = txMessage.getSenderAddress().toBytes();
        byte[] nonce = txMessage.getNonce();
        Address new_contract = new Address(NativeKernelInterface.contract_address(sender, nonce));
        if (Constants.DEBUG)
            System.out.println(new_contract);
        return new_contract;
    }
}