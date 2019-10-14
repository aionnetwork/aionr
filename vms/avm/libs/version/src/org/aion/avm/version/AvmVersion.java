package org.aion.avm.version;

import java.io.IOException;
import java.lang.reflect.Method;
import java.io.ByteArrayOutputStream;
import java.io.ObjectOutput;
import java.io.ObjectOutputStream;

public class AvmVersion {
    private static AvmResourcesV1 resource_v1;
    private static AvmResourcesV2 resource_v2;

    public static void main(String[] args) {
        System.out.println("Hello");
    }

    public static void init_avm_with_version(int version, String root_path) {
        System.out.println("AvmVersion: start init avm resources");
        if (version == 0) {
            try {
                AvmResourcesV1.loadResources(root_path);
            } catch (Exception e) {
                resource_v1 = null;
            }
            resource_v2 = null;
        } else {
            resource_v1 = null;
            try {
                AvmResourcesV2.loadResources(root_path);
            } catch (Exception e) {
                resource_v2 = null;
            }
        }
    }

    // call this method once avm execution is done
    public static void closeAvmResources() {
        if (resource_v1 != null) {
            try {
                resource_v1.close();
            } catch (Exception e) {
                // TODO: handle this exception
            }
        }
        if (resource_v2 != null) {
            try {
                resource_v2.close();
            } catch (Exception e) {
                // TODO: handle this exception
            }
        }
    }

    private static byte[] convertToBytes(Object object) {
        try {
            ByteArrayOutputStream bos = new ByteArrayOutputStream();
            ObjectOutput out = new ObjectOutputStream(bos);
            out.writeObject(object);
            return bos.toByteArray();
        } catch (IOException e) {
            e.printStackTrace();
            return new byte[0];
        }
    }

    public static byte[] execute(
            int version,
            String root_path,
            long handle,
            byte[] txs,
            boolean is_local)
    {
        try {
            if (version == 0) {
                if (resource_v2 != null) {
                    resource_v2.close();
                    resource_v2 = null;
                    //System.gc();
                }
                if (resource_v1 == null) {
                    resource_v1 = AvmResourcesV1.loadResources(root_path);
                }
                Class<?> clazz = resource_v1.loadClass(AvmDependencyInfo.avmExecutor);
                Method[] methods = clazz.getDeclaredMethods();
                Method callMethod = null;
                for(Method method:methods){
                    if( method.getName().equals("execute")) {
                        callMethod = method;
                        break;
                    }
                }
                callMethod.setAccessible(true);

                return (byte[])callMethod.invoke(null, handle, txs, is_local);
            } else {
                if (resource_v1 != null) {
                    System.out.println("AvmVersion: close v1");
                    resource_v1.close();
                    resource_v1 = null;
                    //System.gc();
                }
                if (resource_v2 == null) {
                    resource_v2 = AvmResourcesV2.loadResources(root_path);
                }
                Class<?> clazz = resource_v2.loadClass(AvmDependencyInfo.avmExecutor);
                Method[] methods = clazz.getDeclaredMethods();
                Method callMethod = null;
                for(Method method:methods){
                    if( method.getName().equals("execute")) {
                        callMethod = method;
                        break;
                    }
                }
                callMethod.setAccessible(true);

                return (byte[])callMethod.invoke(null, handle, txs, is_local);
            }
        } catch (Exception e) {
            e.printStackTrace();
            return new byte[0];
        }
    }
}
