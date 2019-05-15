package org.aion.avm.jni;

import org.aion.vm.api.interfaces.KernelInterface;
import org.aion.vm.api.interfaces.SimpleFuture;
import org.aion.vm.api.interfaces.TransactionInterface;
import org.aion.vm.api.interfaces.TransactionResult;
import org.aion.vm.api.interfaces.IExecutionLog;
import org.aion.vm.api.interfaces.TransactionSideEffects;
import org.aion.avm.core.AvmImpl;
import org.aion.avm.core.CommonAvmFactory;
import org.aion.avm.core.IExternalCapabilities;
import org.aion.avm.core.AvmConfiguration;
import org.aion.avm.tooling.StandardCapabilities;
import org.aion.kernel.AvmTransactionResult;
import org.aion.kernel.TransactionalKernel;

import java.util.Set;

public class NativeTransactionExecutor {

    /**
     * Runs the given transactions with the specified handle, and returns the transaction results
     *
     * @param handle reserved pointer for the client
     * @param txs    serialized list of transaction contexts, using the Native Codec
     * @return serialized list of transaction result, using the Native Codec
     */
    public static byte[] execute(long handle, byte[] txs, boolean is_local) {
        try {
            // deserialize the transaction contexts
            // the paralleled transactions should have the same block info

            // submit the transactions to a newly created avm for execution
            NativeKernelInterface kernel = new NativeKernelInterface(handle, is_local);
            Substate substate = new Substate(kernel, is_local);

            NativeDecoder decoder = new NativeDecoder(txs);
            TransactionInterface[] contexts = new TransactionInterface[decoder.decodeInt()];
            for (int i = 0; i < contexts.length; i++) {
                Message msg = new Message(decoder.decodeBytes());
                substate.updateEnvInfo(msg);
                contexts[i] = msg; 
                if (Constants.DEBUG)
                    System.out.println(contexts[i]);
            }

            AvmConfiguration config = new AvmConfiguration();
            if (Constants.DEBUG)
                config.enableVerboseConcurrentExecutor = true;
            AvmImpl avm = CommonAvmFactory.buildAvmInstanceForConfiguration(new AionCapabilities(), config);
            SimpleFuture<TransactionResult>[] futures = avm.run(substate, contexts);

            // wait for the transaction results and serialize them into bytes
            NativeEncoder encoder = new NativeEncoder();
            encoder.encodeInt(futures.length);
            for (int i = 0; i < futures.length; i++) {
                TransactionResult r = futures[i].get();
                encoder.encodeBytes(TransactionResultHelper.encodeTransactionResult(r));
                if (Constants.DEBUG) {
                    System.out.println(futures[i]);
                }
                //TODO: get VM kernel interface generated during execution; then update substates
                KernelInterface transactionKernel = r.getKernelInterface();
                // for (byte[] addr: transactionKernel.getTouchedAccounts()) {
                //     kernel.touchAccount(addr, i);
                // }
                
                byte[] state_root;
                if (is_local) {
                    state_root = kernel.sendSignal(1);
                } else {
                    TransactionSideEffects sideEffects = r.getSideEffects();
                    for (IExecutionLog log: sideEffects.getExecutionLogs()) {
                        NativeEncoder logEncoder = new NativeEncoder();
                        logEncoder.encodeBytes(log.getSourceAddress().toBytes());
                        logEncoder.encodeInt(log.getTopics().size());
                        for (byte[] topic: log.getTopics()) {
                            logEncoder.encodeBytes(topic);
                        }
                        logEncoder.encodeBytes(log.getData());
                        kernel.addLog(logEncoder.toByteArray(), i);
                    }
                    transactionKernel.commitTo(kernel);
                    // 0: should commit state; and return state root
                    state_root = kernel.sendSignal(0);
                }
                   
                encoder.encodeBytes(state_root);
            }
            kernel.sendSignal(-1);
            avm.shutdown();

            return encoder.toByteArray();
        } catch (Exception e) {
            // instead of propagating the exceptions to client, we dump it from the java side
            // and return NULL to indicate an error.
            e.printStackTrace();
            return null;
        }
    }
}
