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

    public static void getAvm(URLClassLoader clsLoader, long handle, byte[] txs, boolean is_local) {
        System.out.println("native exec: getAvm");
        long blockNumber = 0;
        AvmConfiguration config = new AvmConfiguration();
        if (Constants.DEBUG)
            config.enableVerboseConcurrentExecutor = true;
        System.out.println(String.format("native exec: new AionCapabilities"));
        AionCapabilitiesV1 cap = new AionCapabilitiesV1();
        System.out.println(String.format("native exec: new AvmImpl"));
        try {
            Class<?> clazz = clsLoader.loadClass("org.aion.avm.core.CommonAvmFactory");
            // clsLoader.loadClass("org.objectweb.asm.ClassVisitor");
            Method[] methods = clazz.getDeclaredMethods();
            Method callMethod = null;
            for(Method method:methods){
                if( method.getName().equals("buildAvmInstanceForConfiguration")) {
                    callMethod = method;
                    System.out.println("native exec: found method " + method.getName());
                    break;
                }
            }
            callMethod.setAccessible(true);

            System.out.println("native exec: invoke method");
            AvmImpl avm = (AvmImpl)callMethod.invoke(null, cap, config);
            // AvmImpl avm = CommonAvmFactory.buildAvmInstanceForConfiguration(cap, config);

            NativeKernelInterface kernel = new NativeKernelInterface(handle, is_local);
            Substate substate = new Substate(kernel, is_local);
            NativeDecoder decoder = new NativeDecoder(txs);
            System.out.println("native exec: generate avm txs");
            Transaction[] contexts = new Transaction[decoder.decodeInt()];
            for (int i = 0; i < contexts.length; i++) {
                Message msg = new Message(decoder.decodeBytes());
                substate.updateEnvInfo(msg);
                contexts[i] = msg.toAvmTransaction(); 
                blockNumber = msg.blockNumber;
                if (Constants.DEBUG)
                    System.out.println(contexts[i]);
            }

            FutureResult[] futures = avm.run(substate, contexts, ExecutionType.ASSUME_MAINCHAIN, blockNumber-1);

            System.out.println("native exec: encode result");
            // wait for the transaction results and serialize them into bytes
            NativeEncoder encoder = new NativeEncoder();
            encoder.encodeInt(futures.length);
            System.out.println(String.format("native exec: future length [%d]", futures.length));
            
            for (int i = 0; i < futures.length; i++) {
                System.out.println("1111");
                TransactionResult r = futures[i].getResult();
                System.out.println("2222");
                encoder.encodeBytes(TransactionResultHelper.encodeTransactionResult(r));
                if (Constants.DEBUG) {
                    System.out.println(futures[i]);
                }
                System.out.println(String.format("native exec: future [%d]", i));
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

            System.out.println(String.format("native exec: new AvmImpl done"));
            // return avm;
        } catch (Exception e) {
            e.printStackTrace();
            // return null;
        }
    }

    private static Object convertFromBytes(byte[] bytes) throws IOException, ClassNotFoundException {
        try {
            ByteArrayInputStream bis = new ByteArrayInputStream(bytes);
            ObjectInput in = new ObjectInputStream(bis);
            return in.readObject();
        } catch (Exception e) {
            e.printStackTrace();
            return null;
        }
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
            System.out.println("JNI V1");
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
            if (Constants.DEBUG)
                config.enableVerboseConcurrentExecutor = true;
            AionCapabilitiesV1 cap = new AionCapabilitiesV1();
            AvmImpl avm = CommonAvmFactory.buildAvmInstanceForConfiguration(cap, config);
            FutureResult[] futures = avm.run(substate, contexts, ExecutionType.ASSUME_MAINCHAIN, blockNumber-1);

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

    /**
     * Runs the given transactions with the specified handle, and returns the transaction results
     *
     * @param handle reserved pointer for the client
     * @param txs    serialized list of transaction contexts, using the Native Codec
     * @return serialized list of transaction result, using the Native Codec
     */
    // public static byte[] execute_v2(long handle, byte[] txs, boolean is_local) {
    //     long blockNumber = 0;
    //     try {
    //         // deserialize the transaction contexts
    //         // the paralleled transactions should have the same block info

    //         // submit the transactions to a newly created avm for execution
    //         NativeKernelInterface kernel = new NativeKernelInterface(handle, is_local);
    //         Substate substate = new Substate(kernel, is_local);

    //         NativeDecoder decoder = new NativeDecoder(txs);
    //         Transaction[] contexts = new Transaction[decoder.decodeInt()];
    //         for (int i = 0; i < contexts.length; i++) {
    //             Message msg = new Message(decoder.decodeBytes());
    //             substate.updateEnvInfo(msg);
    //             contexts[i] = msg.toAvmTransaction(); 
    //             blockNumber = msg.blockNumber;
    //             if (Constants.DEBUG)
    //                 System.out.println(contexts[i]);
    //         }

    //         AvmConfiguration config = new AvmConfiguration();
    //         if (Constants.DEBUG)
    //             config.enableVerboseConcurrentExecutor = true;
    //         AvmImpl avm = CommonAvmFactory.buildAvmInstanceForConfiguration(new AionCapabilitiesV2(), config);
    //         FutureResult[] futures = avm.run(substate, contexts, ExecutionType.ASSUME_MAINCHAIN, blockNumber-1);

    //         // wait for the transaction results and serialize them into bytes
    //         NativeEncoder encoder = new NativeEncoder();
    //         encoder.encodeInt(futures.length);
    //         for (int i = 0; i < futures.length; i++) {
    //             TransactionResult r = futures[i].getResult();
    //             encoder.encodeBytes(TransactionResultHelper.encodeTransactionResult(r));
    //             if (Constants.DEBUG) {
    //                 System.out.println(futures[i]);
    //             }
    //             //TODO: get VM kernel interface generated during execution; then update substates
    //             IExternalState transactionKernel = futures[i].getExternalState();
    //             // for (byte[] addr: transactionKernel.getTouchedAccounts()) {
    //             //     kernel.touchAccount(addr, i);
    //             // }
                
    //             byte[] state_root;
    //             if (is_local) {
    //                 state_root = kernel.sendSignal(1);
    //             } else {
    //                 for (Log log: r.logs) {
    //                     NativeEncoder logEncoder = new NativeEncoder();
    //                     logEncoder.encodeBytes(log.copyOfAddress());
    //                     List<byte[]> topics = log.copyOfTopics();
    //                     logEncoder.encodeInt(topics.size());
    //                     for (byte[] topic: topics) {
    //                         logEncoder.encodeBytes(topic);
    //                     }
    //                     logEncoder.encodeBytes(log.copyOfData());
    //                     kernel.addLog(logEncoder.toByteArray(), i);
    //                 }
    //                 transactionKernel.commitTo(kernel);
    //                 // 0: should commit state; and return state root
    //                 state_root = kernel.sendSignal(0);
    //             }
                   
    //             encoder.encodeBytes(state_root);
    //         }
    //         kernel.sendSignal(-1);
    //         avm.shutdown();

    //         return encoder.toByteArray();
    //     } catch (Exception e) {
    //         // instead of propagating the exceptions to client, we dump it from the java side
    //         // and return NULL to indicate an error.
    //         e.printStackTrace();
    //         return null;
    //     }
    // }
}
