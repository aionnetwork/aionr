package org.aion.avm.jni;

import org.aion.types.TransactionResult;
import org.aion.types.TransactionStatus;
import org.aion.types.InternalTransaction;

import java.util.List;
import java.util.Optional;

public class TransactionResultHelper {

    /**
     * Transaction result code can be divided into three categories:
     * <p>
     * <ul>
     * <li>Success - Everything is good;</li>
     * <li>Rejection - The transaction should be rejected, due to failure on validation rules;</li>
     * <li>Failure - The transaction failed in the VM, but should be included on chain.</li>
     * </ul>
     * <p>
     * TODO: discuss with the Java kernel team regarding this specs.
     *
     * @param code
     * @return
     */
    public static int encodeAvmResultCode(TransactionStatus code) {
        if (code.isFailed()) {
            return 2;
        } else if (code.isRejected()) {
            return 1;
        } else if (code.isSuccess()) {
            return 0;
        } else {
            // it's a fatal error
            return 255;
        }
    }

    public static byte[] encodeTransactionResult(TransactionResult result) {
        NativeEncoder enc = new NativeEncoder();

        // AvmTransactionResult avm_result = (AvmTransactionResult) result;
        // ResultCode code = result.getResultCode();
        enc.encodeInt(encodeAvmResultCode(result.transactionStatus));
        Optional<byte[]> output = result.copyOfTransactionOutput();
        // enc.encodeBytes(result.output == null ? new byte[0] : result.output);
        enc.encodeBytes(output.orElse(new byte[0]));
        enc.encodeLong(result.energyUsed);

        // Encode invokable internal transactions
        for (InternalTransaction tx: result.internalTransactions) {
            if (tx.copyOfInvokableHash() != null) {
                enc.encodeBytes(tx.copyOfInvokableHash());
            }
        }

        return enc.toByteArray();
    }
}
