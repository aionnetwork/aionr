package org.aion.avm.jni;

import org.aion.avm.core.IExternalCapabilities;
import org.aion.types.Address;
import org.aion.vm.api.interfaces.TransactionContext;
import org.aion.vm.api.interfaces.TransactionInterface;

public class AionCapabilities implements IExternalCapabilities {

    @Override
    public byte[] sha256(byte[] data) {
        return NativeKernelInterface.sha256(data);
    }

    @Override
    public byte[] blake2b(byte[] data) {
        return NativeKernelInterface.blake2b(data);
    }

    @Override
    public byte[] keccak256(byte[] data) {
        return NativeKernelInterface.keccak256(data);
    }

    @Override
    public boolean verifyEdDSA(byte[] data, byte[] data1, byte[] data2) {
        return NativeKernelInterface.edverify(data, data1, data2);
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