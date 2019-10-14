package org.aion.avm.jni;

import org.aion.avm.core.IExternalCapabilities;
import org.aion.types.AionAddress;
import org.aion.types.Transaction;
import org.aion.avm.loader.Loader;

public class AionCapabilitiesV1 implements IExternalCapabilities {

    @Override
    public byte[] sha256(byte[] data) {
        return Loader.sha256(data);
    }

    @Override
    public byte[] blake2b(byte[] data) {
        return Loader.blake2b(data);
    }

    @Override
    public byte[] keccak256(byte[] data) {
        return Loader.keccak256(data);
    }

    @Override
    public boolean verifyEdDSA(byte[] data, byte[] data1, byte[] data2) {
        return Loader.edverify(data, data1, data2);
    }

    @Override
    public AionAddress generateContractAddress(Transaction tx) {
        byte[] sender = tx.senderAddress.toByteArray();
        byte[] nonce = tx.nonce.toByteArray();
        AionAddress new_contract = new AionAddress(Loader.contract_address(sender, nonce));
        return new_contract;
    }
}