package org.aion.avm.jni;

import java.net.URLClassLoader;
import java.util.List;
import java.lang.reflect.Method;

import org.aion.avm.core.AvmImpl;
import org.aion.avm.core.CommonAvmFactory;
import org.aion.avm.core.IExternalCapabilities;
import org.aion.avm.core.AvmConfiguration;
import org.aion.avm.core.ExecutionType;
import org.aion.avm.core.IExternalState;
import org.aion.avm.core.FutureResult;
import org.aion.types.Transaction;
import org.aion.types.TransactionResult;
import org.aion.types.Log;
import org.objectweb.asm.ClassVisitor;

import java.util.Set;
import java.io.IOException;
import java.io.ByteArrayInputStream;
import java.io.ObjectInput;
import java.io.ObjectInputStream;

public class NativeTransactionExecutor {
    public static byte[] test_invoke(long handle, byte[] txs, boolean is_local) {
        System.out.println("hello, invoker");
        return new byte[0];
    }

    /**
     * Runs the given transactions with the specified handle, and returns the transaction results
     *
     * @param handle reserved pointer for the client
     * @param txs    serialized list of transaction contexts, using the Native Codec
     * @return serialized list of transaction result, using the Native Codec
     */
    public static byte[] execute(long handle, byte[] txs, boolean is_local) {
        if (Constants.DEBUG) {
            System.out.println("JNI V2");
        }
        
        long blockNumber = 0;
        try {
            // deserialize the transaction contexts
            // the paralleled transactions should have the same block info

            // submit the transactions to a newly created avm for execution
            NativeKernelInterface kernel = new NativeKernelInterface(handle, is_local);
            Substate substate = new Substate(kernel, is_local);
            NativeDecoder decoder = new NativeDecoder(txs);
            Transaction[] contexts = new Transaction[decoder.decodeInt()];
            for (int i = 0; i < contexts.length; i++) {
                Message msg = new Message(decoder.decodeBytes());
                substate.updateEnvInfo(msg);
                contexts[i] = msg.toAvmTransaction(); 
                blockNumber = msg.blockNumber;
                if (Constants.DEBUG)
                    System.out.println(contexts[i]);
            }

            AvmConfiguration config = new AvmConfiguration();
            // special case for AKI-638 and AKI-644
            if (blockNumber != 4966823 && blockNumber != 5109941) {
                config.enableCoinbaseLocking = true;
            }
            if (Constants.DEBUG) {
                config.enableVerboseContractErrors = true;
                config.enableVerboseConcurrentExecutor = true;
            }
            AionCapabilitiesV2 cap = new AionCapabilitiesV2();
            AvmImpl avm = CommonAvmFactory.buildAvmInstanceForConfiguration(cap, config);

            FutureResult[] futures = avm.run(substate, contexts, ExecutionType.ASSUME_MAINCHAIN , blockNumber-1);

            // wait for the transaction results and serialize them into bytes
            NativeEncoder encoder = new NativeEncoder();
            encoder.encodeInt(futures.length);
            for (int i = 0; i < futures.length; i++) {
                TransactionResult r = futures[i].getResult();
                encoder.encodeBytes(TransactionResultHelper.encodeTransactionResult(r));
                if (Constants.DEBUG) {
                    System.out.println(futures[i]);
                }
                //TODO: get VM kernel interface generated during execution; then update substates
                IExternalState transactionKernel = futures[i].getExternalState();
                
                byte[] state_root;
                if (is_local) {
                    state_root = kernel.sendSignal(1);
                } else {
                    for (Log log: r.logs) {
                        NativeEncoder logEncoder = new NativeEncoder();
                        logEncoder.encodeBytes(log.copyOfAddress());
                        List<byte[]> topics = log.copyOfTopics();
                        logEncoder.encodeInt(topics.size());
                        for (byte[] topic: topics) {
                            logEncoder.encodeBytes(topic);
                        }
                        logEncoder.encodeBytes(log.copyOfData());
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
