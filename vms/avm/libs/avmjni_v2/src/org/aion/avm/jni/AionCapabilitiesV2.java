package org.aion.avm.jni;

import org.aion.avm.core.IExternalCapabilities;
import org.aion.types.AionAddress;
import org.aion.types.Transaction;
import org.aion.types.InternalTransaction;
import org.aion.avm.jni.NativeKernelInterface;
import org.aion.avm.utils.InvokableTxUtil;
import org.aion.avm.loader.Loader;

import java.math.BigInteger;

public class AionCapabilitiesV2 implements IExternalCapabilities {

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
    public AionAddress generateContractAddress(AionAddress deployerAddress, BigInteger nonce) {
        // byte[] sender = deployerAddress.toByteArray();
        // byte[] nonce = nonce.toByteArray();
        AionAddress new_contract = new AionAddress(Loader.contract_address(deployerAddress.toByteArray(), nonce.toByteArray()));
        return new_contract;
    }

    @Override
    public InternalTransaction decodeSerializedTransaction(byte[] innerTx, AionAddress executor, long energyPrice, long energyLimit) {
            return InvokableTxUtil.decode(innerTx, executor, energyPrice, energyLimit);
    }
}