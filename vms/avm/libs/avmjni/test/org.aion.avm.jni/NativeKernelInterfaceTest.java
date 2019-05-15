package org.aion.avm.jni;

import org.junit.Test;

public class NativeKernelInterfaceTest {
    @Test
    public void testClassInitialization() throws ClassNotFoundException {
        Class.forName("org.aion.avm.jni.NativeKernelInterface", true, this.getClass().getClassLoader());
    }
}
