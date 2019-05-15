package org.aion.avm.jni;

import org.aion.kernel.Log;
import org.aion.kernel.AvmTransactionResult;
import org.aion.vm.api.interfaces.TransactionResult;
import org.aion.vm.api.interfaces.ResultCode;

import java.util.List;

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
    public static int encodeAvmResultCode(ResultCode code) {
        if (code.isFailed()) {
            return 2;
        } else if (code.isRejected()) {
            return 1;
        } else if (code.isSuccess()) {
            return 0;
        } else {
            // it's a fatal error
            return -1;
        }
    }

    public static byte[] encodeTransactionResult(TransactionResult result) {
        NativeEncoder enc = new NativeEncoder();

        AvmTransactionResult avm_result = (AvmTransactionResult) result;
        ResultCode code = avm_result.getResultCode();
        enc.encodeInt(encodeAvmResultCode(code));
        enc.encodeBytes(result.getReturnData() == null ? new byte[0] : result.getReturnData());
        enc.encodeLong(avm_result.getEnergyUsed());

        return enc.toByteArray();
    }
}
