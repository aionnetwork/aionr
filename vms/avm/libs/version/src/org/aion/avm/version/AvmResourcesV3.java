package org.aion.avm.version;

import java.io.Closeable;
import java.io.File;
import java.io.IOException;
import java.net.MalformedURLException;
import java.net.URL;
import java.net.URLClassLoader;
import org.aion.avm.version.AvmDependencyInfo;

/**
 * A class that provides access to AVM version 1 resources.
 *
 * This class should be closed once it is finished with so that resources are not leaked.
 *
 * This class is not thread-safe!
 *
 * @implNote Closing an instance of this class will close the unique {@link ClassLoader} that all
 * of the resources were loaded in. This means that any new resources not already acquired cannot
 * be acquired. To be safe, this should only be closed once all resources are completely done with.
 */
public final class AvmResourcesV3 implements Closeable {
    private final URLClassLoader classLoader;
    // public final IAvmResourceFactory resourceFactory;
    // private AionVirtualMachine avm;

    private AvmResourcesV3(URLClassLoader classLoader) {
        this.classLoader = classLoader;
    }

    /**
     * Loads the resources associated with version 1 of the avm and returns a new instance of this
     * resource-holder class.
     */
    public static AvmResourcesV3 loadResources(String projectRootDir) throws IllegalAccessException, ClassNotFoundException, InstantiationException, IOException {
        System.out.println("AvmVersoin3: load resources from " + projectRootDir);
        URLClassLoader classLoader = newClassLoaderForAvmVersion3(projectRootDir);
        return new AvmResourcesV3(classLoader);
    }

    public Class<?> loadClass(String class_name) throws ClassNotFoundException {
        return this.classLoader.loadClass(class_name);
    }

    /**
     * Closes the resources associated with this object.
     */
    @Override
    public void close() throws IOException {
        this.classLoader.close();
    }

    /**
     * Loads all of the required dependencies that are unique to version 1 of the avm in a new
     * classloader and returns this classloader.
     *
     * @return the classloader with the version 1 dependencies.
     */
    private static URLClassLoader newClassLoaderForAvmVersion3(String projectRootPath) throws MalformedURLException {
        File avmCoreJar = new File(projectRootPath + AvmDependencyInfo.coreJarPathVersion3);
        File avmRtJar = new File(projectRootPath + AvmDependencyInfo.rtJarPathVersion3);
        File avmUserlibJar = new File(projectRootPath + AvmDependencyInfo.userlibJarPathVersion3);
        File avmApiJar = new File(projectRootPath + AvmDependencyInfo.apiJarPathVersion3);
        File rustJniJar = new File(projectRootPath + AvmDependencyInfo.rustJniVersion3);
        File aionTypes = new File(projectRootPath + AvmDependencyInfo.aionTypes);
        File asm = new File(projectRootPath + org.aion.avm.version.AvmDependencyInfo.asm);
        File asm_analysis = new File(projectRootPath + org.aion.avm.version.AvmDependencyInfo.asm_analysis);
        File asm_common = new File(projectRootPath + AvmDependencyInfo.asm_common);
        File asm_tree = new File(projectRootPath + AvmDependencyInfo.asm_tree);
        File asm_util = new File(projectRootPath + AvmDependencyInfo.asm_util);
        File slf4j_api = new File(projectRootPath + AvmDependencyInfo.slf4j_api);
        File slf4j_simple = new File(projectRootPath + AvmDependencyInfo.slf4j_simple);
        File spongycastle = new File(projectRootPath + org.aion.avm.version.AvmDependencyInfo.spongycastle);
        File hamcrest = new File(projectRootPath + AvmDependencyInfo.hamcrest);
        File utilities = new File(projectRootPath + AvmDependencyInfo.utils);
        URL[] urls = new URL[]{
                avmCoreJar.toURI().toURL(),
                avmRtJar.toURI().toURL(),
                avmUserlibJar.toURI().toURL(),
                avmApiJar.toURI().toURL(),
                rustJniJar.toURI().toURL(),
                aionTypes.toURI().toURL(),
                asm.toURI().toURL(),
                asm_tree.toURI().toURL(),
                asm_common.toURI().toURL(),
                asm_analysis.toURI().toURL(),
                asm_util.toURI().toURL(),
                slf4j_api.toURI().toURL(),
                slf4j_simple.toURI().toURL(),
                spongycastle.toURI().toURL(),
                hamcrest.toURI().toURL(),
                utilities.toURI().toURL()
        };
        return new URLClassLoader(urls);
    }
}