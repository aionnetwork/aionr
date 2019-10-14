package org.aion.avm.version;

public interface IExecutor {
    /*
        execute avm version 1
     */
    public byte[] execute(long handle, byte[] txs, boolean is_local);
    /*
        Execute avm version 2
     */
    public byte[] execute_v2(long handle, byte[] txs, boolean is_local);
}
