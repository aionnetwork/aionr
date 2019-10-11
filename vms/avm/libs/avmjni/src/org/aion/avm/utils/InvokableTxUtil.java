package org.aion.avm.utils;

import java.math.BigInteger;
import java.util.Arrays;

// import org.aion.crypto.ECKey;
// import org.aion.crypto.HashUtil;
// import org.aion.crypto.ISignature;
// import org.aion.crypto.SignatureFac;
import org.aion.types.AionAddress;
import org.aion.types.InternalTransaction;
import org.aion.types.InternalTransaction.RejectedStatus;
import org.aion.types.Transaction;
import org.aion.avm.jni.NativeKernelInterface;;
// import org.aion.util.types.AddressUtils;

public class InvokableTxUtil {

    private static final int
        RLP_META_TX_NONCE = 0,
        RLP_META_TX_TO = 1,
        RLP_META_TX_VALUE = 2,
        RLP_META_TX_DATA = 3,
        RLP_META_TX_EXECUTOR = 4,
        RLP_META_TX_SIG = 5;

    // public static byte[] encodeInvokableTransaction(
    //         ECKey key,
    //         BigInteger nonce,
    //         AionAddress destination,
    //         BigInteger value,
    //         byte[] data,
    //         AionAddress executor) {

    //     byte[] rlpEncodingWithoutSignature =
    //         rlpEncodeWithoutSignature(
    //             nonce,
    //             destination,
    //             value,
    //             data,
    //             executor);

    //     ISignature signature = key.sign(HashUtil.h256(rlpEncodingWithoutSignature));

    //     return rlpEncode(
    //         nonce,
    //         destination,
    //         value,
    //         data,
    //         executor,
    //         signature);
    // }

    private static byte[] rlpEncodeWithoutSignature(
            BigInteger nonce,
            AionAddress destination,
            BigInteger value,
            byte[] data,
            AionAddress executor) {

        byte[] nonceEncoded = RLP.encodeBigInteger(nonce);
        byte[] destinationEncoded = RLP.encodeElement(destination == null ? null : destination.toByteArray());
        byte[] valueEncoded = RLP.encodeBigInteger(value);
        byte[] dataEncoded = RLP.encodeElement(data);
        byte[] executorEncoded = RLP.encodeElement(executor == null ? null : executor.toByteArray());

        return RLP.encodeList(
            nonceEncoded,
            destinationEncoded,
            valueEncoded,
            dataEncoded,
            executorEncoded);
    }

    // private static byte[] rlpEncode(
    //         BigInteger nonce,
    //         AionAddress destination,
    //         BigInteger value,
    //         byte[] data,
    //         AionAddress executor,
    //         ISignature signature) {

    //     byte[] nonceEncoded = RLP.encodeBigInteger(nonce);
    //     byte[] destinationEncoded = RLP.encodeElement(destination == null ? null : destination.toByteArray());
    //     byte[] valueEncoded = RLP.encodeBigInteger(value);
    //     byte[] dataEncoded = RLP.encodeElement(data);
    //     byte[] executorEncoded = RLP.encodeElement(executor == null ? null : executor.toByteArray());
    //     byte[] signatureEncoded = RLP.encodeElement(signature.toBytes());

    //     return RLP.encodeList(
    //         nonceEncoded,
    //         destinationEncoded,
    //         valueEncoded,
    //         dataEncoded,
    //         executorEncoded,
    //         signatureEncoded);
    // }


    public static InternalTransaction decode(byte[] rlpEncoding, AionAddress callingAddress, long energyPrice, long energyLimit) {

        RLPList decodedTxList;
        try {
            decodedTxList = RLP.decode2(rlpEncoding);
        } catch (Exception e) {
            return null;
        }
        RLPList tx = (RLPList) decodedTxList.get(0);

        BigInteger nonce = new BigInteger(1, tx.get(RLP_META_TX_NONCE).getRLPData());
        BigInteger value = new BigInteger(1, tx.get(RLP_META_TX_VALUE).getRLPData());
        byte[] data = tx.get(RLP_META_TX_DATA).getRLPData();

        AionAddress destination;
        try {
            destination = new AionAddress(tx.get(RLP_META_TX_TO).getRLPData());
        } catch(Exception e) {
            destination = null;
        }

        AionAddress executor;
        try {
            executor = new AionAddress(tx.get(RLP_META_TX_EXECUTOR).getRLPData());
        } catch(Exception e) {
            executor = null;
        }

        // Verify the executor

        if (executor != null && !executor.equals(AddressUtils.ZERO_ADDRESS) && !executor.equals(callingAddress)) {
            return null;
        }

        byte[] sigs = tx.get(RLP_META_TX_SIG).getRLPData();
        // ISignature signature;
        AionAddress sender;
        if (sigs != null) {
            // Singature Factory will decode the signature based on the algo
            // presetted in main() entry.
            // ISignature is = SignatureFac.fromBytes(sigs);
            // if (is != null) {
            //     signature = is;
            //     sender = new AionAddress(is.getAddress());
            // } else {
            //     return null;
            // }
            // TODO: use signature method in rust kernel
            if (sigs.length != 96) {
                return null;
            } else {
                sender = new AionAddress(Arrays.copyOfRange(sigs, 0, 32));
            }
        } else {
            return null;
        }

        try {
            return createFromRlp(
                nonce,
                sender,
                destination,
                value,
                data,
                executor,
                energyLimit,
                energyPrice,
                sigs,
                rlpEncoding);
        }
        catch (Exception e) {
            return null;
        }
    }

    private static InternalTransaction createFromRlp(
            BigInteger nonce,
            AionAddress sender,
            AionAddress destination,
            BigInteger value,
            byte[] data,
            AionAddress executor,
            long energyLimit,
            long energyPrice,
            byte[] signature,
            byte[] rlpEncoding) {

        byte[] transactionHashWithoutSignature =
        NativeKernelInterface.blake2b(
                InvokableTxUtil.rlpEncodeWithoutSignature(
                    nonce,
                    destination,
                    value,
                    data,
                    executor));

        // message, public_key, signature
        if (!NativeKernelInterface.edverify(transactionHashWithoutSignature, 
            Arrays.copyOfRange(signature, 0, 32),
            Arrays.copyOfRange(signature, 32, 96)))
        {
            throw new IllegalStateException("Signature does not match Transaction Content");
        }

        byte[] transactionHash = NativeKernelInterface.blake2b(rlpEncoding);

        if (destination == null) {
            return
                InternalTransaction.contractCreateMetaTransaction(
                    RejectedStatus.NOT_REJECTED,
                    sender,
                    nonce,
                    value,
                    data,
                    energyLimit,
                    energyPrice,
                    transactionHash);
        } else {
            return
                InternalTransaction.contractCallMetaTransaction(
                    RejectedStatus.NOT_REJECTED,
                    sender,
                    destination,
                    nonce,
                    value,
                    data,
                    energyLimit,
                    energyPrice,
                    transactionHash);
        }
    }
}
