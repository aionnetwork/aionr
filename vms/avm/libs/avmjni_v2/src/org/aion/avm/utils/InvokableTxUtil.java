package org.aion.avm.utils;

import java.math.BigInteger;
import java.util.Arrays;
import java.nio.ByteBuffer;

import org.aion.types.AionAddress;
import org.aion.types.InternalTransaction;
import org.aion.types.InternalTransaction.RejectedStatus;
import org.aion.types.Transaction;
import org.aion.avm.jni.NativeKernelInterface;
import org.aion.avm.loader.Loader;
import org.aion.avm.rlp.RLPList;
import org.aion.avm.rlp.RLP;

public class InvokableTxUtil {

    private static final int
        RLP_META_TX_NONCE = 0,
        RLP_META_TX_TO = 1,
        RLP_META_TX_VALUE = 2,
        RLP_META_TX_DATA = 3,
        RLP_META_TX_EXECUTOR = 4,
        RLP_META_TX_SIG = 5;
    
    private static final byte VERSION = 0;

    private static final byte A0_IDENTIFIER = ByteUtil.hexStringToBytes("0xA0")[0];

    /**
     * Returns an address of with identifier A0, given the public key of the account (this is
     * currently our only account type)
     */
    private static byte[] computeA0Address(byte[] publicKey) {
        ByteBuffer buf = ByteBuffer.allocate(32);
        buf.put(A0_IDENTIFIER);
        // [1:]
        buf.put(Loader.blake2b(publicKey), 1, 31);
        return buf.array();
    }

    private static byte[] prependVersion(byte[] encoding) {
        byte[] ret = new byte[encoding.length + 1];
        ret[0] = VERSION;
        System.arraycopy(encoding, 0, ret, 1, encoding.length);
        return ret;
    }

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


    public static InternalTransaction decode(byte[] rlpEncodingWithVersion, AionAddress callingAddress, long energyPrice, long energyLimit) {
        if (rlpEncodingWithVersion[0] != 0) { return null; }

        byte[] rlpEncoding = Arrays.copyOfRange(rlpEncodingWithVersion, 1, rlpEncodingWithVersion.length);
        
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
            if (sigs.length != 96) {
                return null;
            } else {
                sender = new AionAddress(computeA0Address(Arrays.copyOfRange(sigs, 0, 32)));
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
            e.printStackTrace();
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
        Loader.blake2b(
            prependVersion(
                InvokableTxUtil.rlpEncodeWithoutSignature(
                    nonce,
                    destination,
                    value,
                    data,
                    executor)));

        // message, public_key, signature
        if (!Loader.edverify(transactionHashWithoutSignature, 
            Arrays.copyOfRange(signature, 0, 32),
            Arrays.copyOfRange(signature, 32, 96)))
        {
            throw new IllegalStateException("Signature does not match Transaction Content");
        }

        byte[] transactionHash = Loader.blake2b(rlpEncoding);

        if (destination == null) {
            return
                InternalTransaction.contractCreateInvokableTransaction(
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
                InternalTransaction.contractCallInvokableTransaction(
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
