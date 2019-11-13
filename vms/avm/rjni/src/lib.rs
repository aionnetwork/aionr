//
//  rjni
//

//! User friendly bindings to the Java Native Interface.
//!
//! # Usage
//!
//! First you'll need to compile your Java source code, either as separate
//! `.class` files, or package them together as a `.jar` archive.
//!
//! You need to make sure you target the Java compiler to the JVM version you
//! plan to use. This is done through the `-target` and `-source` command line
//! arguments to `javac`.
//!
//! For example, if you have a `/path/to/project/com/me/Test.java` file (ie.
//! the class `com.me.Test`) and you intend to target the 1.6 JVM:
//!
//! ```bash
//! $ javac -target 1.6 -source 1.6 /path/to/project/com/me/Test.java
//! ```
//!
//! This will create a `/path/to/project/com/me/Test.class` file.
//!
//! Then when you create the JVM in Rust, you need to add `/path/to/project`
//! (ie.  the directory containing the root of your Java code) to the classpath,
//! and specify the correct JVM version:

#![allow(dead_code)]

extern crate libc;

pub mod ffi;

use std::path::{PathBuf, Path};
use std::ffi::{CString, CStr};
use std::{mem, ptr, error, fmt, env, char};

/// All possible versions of the JVM.
///
/// Not all of these versions may actually be available, and an "unsupported
/// version" error may be triggered upon creating the JVM.
///
/// The integer values of these versions correspond to the FFI version numbers
/// required by the JVM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Version {
    V11 = 0x00010001,
    V12 = 0x00010002,
    V14 = 0x00010004,
    V15 = 0x00010005,
    V16 = 0x00010006,
    V17 = 0x00010007,
    V18 = 0x00010008,
}

/// Initialisation options required upon creation of the JVM.
pub struct Options {
    version: Version,
    classpath: Classpath,
    initial_heap_size: usize,
    max_heap_size: usize,
    ignore_unrecognised: bool,
    custom: Vec<String>,

    /// This is required in order to preserve the existence of the heap
    /// allocated CString instance, to prevent it from being dropped while a
    /// pointer to its contents is used in the JavaVMInitArgs list.
    option_strings: Vec<CString>,
    options: Vec<ffi::JavaVMOption>,
}

impl Options {
    /// Create an empty set of options.
    pub fn new() -> Options {
        Options {
            version: Version::V11,
            classpath: Classpath::new(),
            initial_heap_size: 0,
            max_heap_size: 0,
            ignore_unrecognised: true,
            custom: Vec::new(),
            option_strings: Vec::new(),
            options: Vec::new(),
        }
    }

    /// Set the JVM version to use.
    pub fn version(mut self, version: Version) -> Options {
        self.version = version;
        self
    }

    /// Set the classpath, which contains a list of filesystem directories that
    /// the JVM will search when looking for a class to load.
    pub fn classpath(mut self, classpath: Classpath) -> Options {
        self.classpath = classpath;
        self
    }

    /// Set the initial heap size for the JVM in bytes.
    ///
    /// Call this with a size of 0 to unset any previously set value.
    pub fn initial_heap_size(mut self, size: usize) -> Options {
        self.initial_heap_size = size;
        self
    }

    /// Set the maximum heap size for the JVM in bytes.
    ///
    /// Call this with a size of 0 to unset any previously set value.
    pub fn max_heap_size(mut self, size: usize) -> Options {
        self.max_heap_size = size;
        self
    }

    /// Set whether the JVM should ignore unrecognised arguments, or trigger an
    /// exception when one is provided.
    pub fn ignore_unrecognized_arguments(mut self, flag: bool) -> Options {
        self.ignore_unrecognised = flag;
        self
    }

    /// Adds a custom, string based option (like passing in a command line
    /// argument to the `java` process).
    pub fn custom<T: ToString>(mut self, arg: T) -> Options {
        self.custom.push(arg.to_string());
        self
    }

    /// Builds the underlying list of options.
    ///
    /// This function is marked unsafe since we use unsafe pointers with regards
    /// to the FFI struct. The caller must ensure that the lifetime of the
    /// options struct is longer than that of the returned arguments struct.
    ///
    /// This function must take a mutable pointer to `self`, rather than consume
    /// self, since the Options struct must outlive the returned JavaVMInitArgs
    /// struct.
    unsafe fn build(&mut self) -> ffi::JavaVMInitArgs {
        // Don't bother specifying heap size configurations if they're equal to
        // 0, as this is the marker value we used
        if self.initial_heap_size > 0 {
            let option = format!("-Xms{}", self.initial_heap_size);
            self.add_option(option);
        }

        if self.max_heap_size > 0 {
            let option = format!("-Xmx{}", self.max_heap_size);
            self.add_option(option);
        }

        // Construct the classpath from a single string, so we only have a
        // single heap allocation (and potentially some future reallocations if
        // the classpath.build function requires it)
        let mut classpath = String::from("-Djava.class.path=");
        self.classpath.build(&mut classpath);
        self.add_option(classpath);

        // Pop each custom option off the list until no more are there, so we
        // don't have to call .clone() on each item in the list and waste heap
        // memory on duplicating a bunch of strings
        while let Some(option) = self.custom.pop() {
            self.add_option(option);
        }

        ffi::JavaVMInitArgs {
            version: mem::transmute(self.version),
            nOptions: self.options.len() as ffi::jint,
            options: self.options.as_mut_ptr(),
            ignoreUnrecognized: self.ignore_unrecognised as ffi::jboolean,
        }
    }

    /// Adds an option to the list of FFI options, used when we're constructing
    /// the final options list.
    fn add_option(&mut self, option: String) {
        let cstr = CString::new(option).unwrap();
        self.options.push(ffi::JavaVMOption {
            optionString: cstr.as_ptr(),
            extraInfo: ptr::null(),
        });

        // Transfer ownership of the CString to the options struct so that it
        // lives for at least as long as the pointer we just created using
        // .as_ptr() above
        self.option_strings.push(cstr);
    }
}

impl Default for Options {
    /// Uses the JNI interface to select the recommended initialisation options
    /// for the JVM.
    ///
    /// Automatically selects the most recently supported version of the JVM on
    /// this system.
    fn default() -> Options {
        // Extract the information from the set of default arguments
        let args = latest_jvm_version().expect("No supported JNI version");
        let version = unsafe { mem::transmute(args.version) };
        let ignore_unrecognised = args.ignoreUnrecognized == ffi::JNI_TRUE;
        Options {
            version: version,
            classpath: Classpath::new(),
            initial_heap_size: 0,
            max_heap_size: 0,
            ignore_unrecognised: ignore_unrecognised,
            custom: Vec::new(),

            option_strings: Vec::new(),
            options: Vec::new(),
        }
    }
}

/// Determines the most recently supported version of the JVM on this system,
/// and returns the JavaVMInitArgs struct for this version, or None if there is
/// no supported version of JVM.
fn latest_jvm_version() -> Option<ffi::JavaVMInitArgs> {
    // The FFI function expects the version field on the args struct to be set
    // before calling, and the return value of the function will indicate if
    // the requested version is supported or not (ie. JNI_OK or JNI_EVERSION).
    //
    // We use this in order to determine the most recently supported JVM
    // version by iterating in reverse order over the versions.
    for version in (Version::V11 as u32..Version::V18 as u32).rev() {
        // Create a default arguments struct with the pre-specified version
        let mut args = ffi::JavaVMInitArgs {
            version: unsafe { mem::transmute(version) },
            nOptions: 0,
            options: ptr::null_mut(),
            ignoreUnrecognized: ffi::JNI_TRUE,
        };

        // Check if this version is supported, indicated by the lack of an
        // error
        let code = unsafe { ffi::JNI_GetDefaultJavaVMInitArgs(&mut args) };
        if code == ffi::JNIError::JNI_OK {
            return Some(args);
        }
    }

    // If we reached here, then there are apparently no supported JNI versions
    // on this system (strangely?)
    None
}

/// A structured list of filesystem directories which the JVM will search when
/// looking for a class to load.
#[derive(Debug)]
pub struct Classpath {
    paths: Vec<PathBuf>,
}

impl Classpath {
    /// Create an empty classpath.
    pub fn new() -> Classpath {
        Classpath {
            paths: Vec::new(),
        }
    }

    /// Add a path to the classpath.
    ///
    /// This can either be the path to a directory containing a number of
    /// compiled .class files, or the direct path to a .jar archive.
    ///
    /// For example, if you're looking for the `Test.class` file in the folder
    /// `/thing`, then you should add the path `/thing` to the classpath. If
    /// you've added the `Test.class` file to a Jar file at `/thing/myjar.jar`,
    /// then you should add the path `/thing/myjar.jar` to the classpath.
    pub fn add<T: AsRef<Path>>(mut self, path: T) -> Classpath {
        self.paths.push(path.as_ref().to_owned());
        self
    }

    /// Builds and returns the underlying classpath string.
    fn build(&self, string: &mut String) {
        // Iterate over each path
        for path in &self.paths {
            let converted_path = path.to_str().unwrap();
            string.push_str(converted_path);

            // The Java classpath separator is different depending on the
            // platform. On Windows it's `;`, on Unix it's `:`
            if env::consts::FAMILY == "windows" {
                string.push(';');
            } else {
                string.push(':');
            }
        }
    }
}

/// An instance of a Java virtual machine.
///
/// Multiple instances of a JVM can be created.
#[derive(Debug, Clone)]
pub struct JavaVM {
    // pub vm: *mut ffi::JavaVM,
    // pub env: *mut ffi::JNIEnv,
    pub vm: i32,
    pub env: i32,
}

impl JavaVM {
    /// Create a new virtual machine from the given set of options.
    pub fn new(mut options: Options) -> Result<JavaVM> {
        unsafe {
            // Construct the FFI options struct
            let mut args = options.build();

            // Create the JVM
            let mut vm = ptr::null_mut();
            let mut env = ptr::null_mut();
            let status = ffi::JNI_CreateJavaVM(&mut vm, &mut env, &mut args);

            // Check for an error
            if status == ffi::JNIError::JNI_OK {
                Ok(JavaVM {
                    vm: ffi::from_raw(vm),
                    env: ffi::from_raw(env),
                })
            } else {
                Err(Error::from_ffi(status))
            }
        }
    }

    /// Load a class by its fully qualified name (including its parent
    /// packages).
    ///
    /// The parent packages of the class should be separated by `/` and not `.`,
    /// as is required by the JNI (eg. `com/ben/Test`, not `com.ben.Test`).
    ///
    /// The JVM searches all directories and Jar files specified in the
    /// classpath (given to the JVM in the Options struct provided upon
    /// initialisation) for a .class file with the given name and parent
    /// packages.
    ///
    /// For example, if the classpath contains the directory `/thing`, and we're
    /// looking for the class `com/ben/Test`, then the JVM will look for a
    /// `/thing/com/ben/Test.class` file.
    ///
    /// This can also be used to load standard Java library files like
    /// `java/lang/String`. Methods can be called on these system classes in
    /// the same way you'd call methods on your custom classes.
    pub fn class<'a>(&'a self, name: &str) -> Result<Class<'a>> {
        // Find the class
        let cstr = CString::new(name).unwrap();
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.env);
        let raw = unsafe { ((**env).FindClass)(env, cstr.as_ptr()) };

        // Check the class exists
        if raw == 0 as ffi::jclass {
            Err(Error::from_exception(self))
        } else {
            // Successfully found the class
            Ok(Class {
                jvm: self,
                raw: raw,
            })
        }
    }

    /// Creates an object array of the given type and length.
    pub fn new_object_array(&self, class_name: &str, size: i32) -> Result<Object> {
        let clazz = self.class(class_name)?;

        let env: *mut ffi::JNIEnv = ffi::into_raw(self.env);
        let raw = unsafe { ((**env).NewObjectArray)(env, size, clazz.raw, ptr::null_mut()) };

        if raw == 0 as ffi::jobject {
            Err(Error::from_exception(self))
        } else {
            Ok(Object {
                jvm: self,
                raw: raw,
            })
        }
    }

    /// Creates a Java byte array
    pub fn new_byte_array(&self, size: i32) -> Result<Object> {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.env);
        let raw = unsafe { ((**env).NewByteArray)(env, size) };

        if raw == 0 as ffi::jobject {
            Err(Error::from_exception(self))
        } else {
            Ok(Object {
                jvm: self,
                raw: raw,
            })
        }
    }

    /// Creates a Java byte array and initialize it the given data
    pub fn new_byte_array_with_data(&self, data: &Vec<u8>) -> Result<Object> {
        let array = self.new_byte_array(data.len() as i32)?;

        let env: *mut ffi::JNIEnv = ffi::into_raw(self.env);
        unsafe {
            let src: *const i8 = mem::transmute(data.as_ptr());
            ((**env).SetByteArrayRegion)(env, array.raw, 0, data.len() as i32, src as *const u8);
        }

        Ok(array)
    }

    /// Read a Java byte array into a Rust vector of u8.
    pub fn load_byte_array(&self, array: &Object) -> Vec<u8> {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.env);
        let size = unsafe { ((**env).GetArrayLength)(env, array.raw) };
        let mut result = vec![0u8; size as usize];
        unsafe {
            ((**env).GetByteArrayRegion)(env, array.raw, 0, size, result.as_mut_ptr() as *mut u8);
        };

        result
    }

    /// Returns true when an exception has occurred.
    fn has_exception(&self) -> bool {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.env);
        unsafe { ((**env).ExceptionCheck)(env) == ffi::JNI_TRUE }
    }

    /// Clears the most recently triggered exception.
    fn clear_exception(&self) {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.env);
        unsafe { ((**env).ExceptionClear)(env) };
    }

    /// Get the throwable instance of the most recently occurred exception.
    fn exception_obj(&self) -> Object {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.env);
        Object {
            jvm: self,
            raw: unsafe { ((**env).ExceptionOccurred)(env) },
        }
    }

    /// Print the current exception, used for debugging purposes.
    fn print_exception(&self) {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.env);
        unsafe { ((**env).ExceptionDescribe)(env) }
    }
}

//
//  Classes and Objects
//

/// The prototype of a class, which can be used to instantiate objects of this
/// class.
///
/// The class has a prescribed lifetime, since it cannot outlive the JVM that
/// created it.
#[derive(Debug, Clone, Copy)]
pub struct Class<'a> {
    jvm: &'a JavaVM,
    raw: ffi::jclass,
}

impl<'a> Class<'a> {
    /// Returns this object's superclass.
    pub fn superclass(&self) -> Class<'a> {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.jvm.env);
        Class {
            jvm: self.jvm,
            raw: unsafe { ((**env).GetSuperclass)(env, self.raw) },
        }
    }

    /// Create an instance of this class.
    ///
    /// The provided arguments are for the object's constructor. The correct
    /// overloaded constructor is chosen based on the types of the arguments.
    pub fn instantiate(&self, args: &[Value]) -> Result<Object<'a>> {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.jvm.env);

        // Get the constructor method ID
        let name = CString::new("<init>").unwrap();
        let signature = CString::new(function_signature(args, &Type::Void)).unwrap();
        let id = unsafe { ((**env).GetMethodID)(env, self.raw, name.as_ptr(), signature.as_ptr()) };

        // Check the constructor exists
        if id == 0 as ffi::jmethodID {
            return Err(Error::from_exception(self.jvm));
        }

        // Convert the list of arguments into an array of jvalues
        let mut java_args = Vec::with_capacity(args.len());
        for arg in args {
            java_args.push(arg.to_jvalue(self.jvm));
        }

        // Call the constructor and instantiate the object
        let obj = unsafe { ((**env).NewObjectA)(env, self.raw, id, java_args.as_ptr()) };

        // Check for an exception
        if self.jvm.has_exception() {
            Err(Error::from_exception(self.jvm))
        } else {
            Ok(Object {
                jvm: self.jvm,
                raw: obj,
            })
        }
    }

    /// Returns the ID for a method with the given name, arguments, and return
    /// type.
    fn method_id(&self, name: &str, args: &[Value], return_type: &Type) -> ffi::jmethodID {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.jvm.env);

        // Get the function signature from the arguments and return type
        let fn_sig = function_signature(args, &return_type);
        let signature = CString::new(fn_sig).unwrap();

        // Convert the name of the method into a useful form
        let name = CString::new(name).unwrap();

        // Call the FFI function
        unsafe { ((**env).GetMethodID)(env, self.raw, name.as_ptr(), signature.as_ptr()) }
    }

    /// Returns the ID for a static method on this class with the given name,
    /// arguments, and return type.
    fn static_method_id(&self, name: &str, args: &[Value], return_type: &Type) -> ffi::jmethodID {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.jvm.env);

        // Get the function signature from the arguments and return type
        let sig = function_signature(args, &return_type);
        let signature = CString::new(sig).unwrap();

        // Convert the name of the method into a useful form
        let name = CString::new(name).unwrap();

        // Call the FFI function
        unsafe { ((**env).GetStaticMethodID)(env, self.raw, name.as_ptr(), signature.as_ptr()) }
    }

    /// Call a static method on this class.
    ///
    /// If the function doesn't return a value (ie. a void return type), then
    /// Value::Void is returned.
    ///
    /// Value::Void should not be passed as an argument, and will generate an
    /// exception.
    pub fn call_static(&self, name: &str, args: &[Value], return_type: Type) -> Result<Value> {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.jvm.env);

        // Get the method ID and check it exists
        let method_id = self.static_method_id(name, args, &return_type);
        if method_id == 0 as ffi::jmethodID {
            return Err(Error::from_exception(self.jvm));
        }

        // Convert the list of arguments into an array of jvalues
        let mut java_args = Vec::with_capacity(args.len());
        for arg in args {
            java_args.push(arg.to_jvalue(self.jvm));
        }

        // Call the method
        let result = unsafe {
            let base: *const ffi::MethodFn = mem::transmute(&(**env).CallStaticObjectMethodA);
            let offset = return_type.offset() * 3;
            let fn_ptr = base.offset(offset as isize);
            (*fn_ptr)(env, self.raw, method_id, java_args.as_ptr())
        };

        // Convert the result into a value
        if self.jvm.has_exception() {
            Err(Error::from_exception(self.jvm))
        } else {
            Ok(Value::from_jvalue(result, &return_type, self.jvm))
        }
    }

    /// Returns the ID for a field with the given name and type.
    fn field_id<T: Signature>(&self, name: &str, kind: &T) -> ffi::jfieldID {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.jvm.env);
        let name = CString::new(name).unwrap();
        let signature = CString::new(kind.signature()).unwrap();
        unsafe { ((**env).GetFieldID)(env, self.raw, name.as_ptr(), signature.as_ptr()) }
    }

    /// Returns the ID for a static field on this class with the given name and
    /// type.
    fn static_field_id<T: Signature>(&self, name: &str, kind: &T) -> ffi::jfieldID {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.jvm.env);
        let name = CString::new(name).unwrap();
        let signature = CString::new(kind.signature()).unwrap();
        unsafe { ((**env).GetStaticFieldID)(env, self.raw, name.as_ptr(), signature.as_ptr()) }
    }

    /// Get the value of a static field on this class.
    pub fn static_field(&self, name: &str, kind: Type) -> Result<Value> {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.jvm.env);

        // Get the field ID and check it exists
        let field_id = self.static_field_id(name, &kind);
        if field_id == 0 as ffi::jfieldID {
            return Err(Error::from_exception(self.jvm));
        }

        // Get the contents of the field
        let result = unsafe {
            let base: *const ffi::GetFieldFn = mem::transmute(&(**env).GetStaticObjectField);
            let offset = kind.offset();
            let fn_ptr = base.offset(offset as isize);
            (*fn_ptr)(env, self.raw, field_id)
        };

        // Convert the result into a value
        if self.jvm.has_exception() {
            Err(Error::from_exception(self.jvm))
        } else {
            Ok(Value::from_jvalue(result, &kind, self.jvm))
        }
    }

    /// Set the value of a static field on this class.
    pub fn set_static_field(&self, name: &str, value: Value) -> Result<()> {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.jvm.env);

        // Get the field ID and check it exists
        let field_id = self.static_field_id(name, &value);
        if field_id == 0 as ffi::jfieldID {
            return Err(Error::from_exception(self.jvm));
        }

        // Convert the value into a useable form
        let java_value = value.to_jvalue(self.jvm);

        // Set the contents of the field
        unsafe {
            let base: *const ffi::SetFieldFn = mem::transmute(&(**env).SetStaticObjectField);
            let offset = value.offset();
            let fn_ptr = base.offset(offset as isize);
            (*fn_ptr)(env, self.raw, field_id, java_value);
        }

        // Convert the result into a value
        if self.jvm.has_exception() {
            Err(Error::from_exception(self.jvm))
        } else {
            Ok(())
        }
    }
}

impl<'a> Into<i32> for Class<'a> {
    fn into(self) -> i32 { unsafe { mem::transmute(&self) } }
}

/// An object (an instance of a class), which can have methods called on it and
/// fields accessed.
///
/// The object has a prescribed lifetime, since it cannot outlive the JVM that
/// created it.
#[derive(Debug)]
pub struct Object<'a> {
    jvm: &'a JavaVM,
    raw: ffi::jobject,
}

impl<'a> Object<'a> {
    /// Returns whether this object is a NULL pointer.
    pub fn is_null(&self) -> bool { self.raw.is_null() }

    /// Returns the fully qualified name of the class this object is an instance
    /// of as a string.
    pub fn class_name(&self) -> Result<String> {
        // Get the corresponding class object
        let class_obj = self
            .call("getClass", &[], Type::Object("java/lang/Class"))?
            .as_object();

        // Call the `getName` method on the class object
        Ok(class_obj.call("getName", &[], Type::Str)?.as_str())
    }

    /// Returns the slash name of the class this object is an instance of.
    pub fn class_name_slash(&self) -> Result<String> {
        let mut name = self.class_name()?;
        name = name.replace(".", "/");
        Ok(name)
    }

    /// Returns the class that this object is an instance of.
    pub fn class(&self) -> Class<'a> {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.jvm.env);
        Class {
            jvm: self.jvm,
            raw: unsafe { ((**env).GetObjectClass)(env, self.raw) },
        }
    }

    /// Returns true if this object is an instance of the given class.
    ///
    /// Both this object and the given class must have been created by the same
    /// JVM instance, otherwise the result of this function is undefined.
    pub fn is_instance_of<'b>(&self, other: Class<'b>) -> bool {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.jvm.env);
        unsafe { ((**env).IsInstanceOf)(env, self.raw, other.raw) == ffi::JNI_TRUE }
    }

    /// Call a method on this object.
    ///
    /// The function's signature is determined by the types of each argument
    /// and the given return type. If the signature doesn't match any valid
    /// function with the given name, then an exception is generated (with no
    /// stack trace).
    ///
    /// If the function doesn't return a value (ie. a void return type), then
    /// Value::Void is returned.
    ///
    /// Value::Void should not be passed as an argument, and will generate an
    /// exception.
    pub fn call(&self, name: &str, args: &[Value], return_type: Type) -> Result<Value> {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.jvm.env);

        // Get the method ID and check it exists
        let class = self.class();
        let method_id = class.method_id(name, args, &return_type);
        if method_id == 0 as ffi::jmethodID {
            return Err(Error::from_exception(self.jvm));
        }

        // Convert the list of arguments into an array of jvalues
        let mut java_args = Vec::with_capacity(args.len());
        for arg in args {
            java_args.push(arg.to_jvalue(self.jvm));
        }

        // Call the method
        let result = unsafe {
            let base: *const ffi::MethodFn = mem::transmute(&(**env).CallObjectMethodA);
            let offset = return_type.offset() * 3;
            let fn_ptr = base.offset(offset as isize);
            (*fn_ptr)(env, self.raw, method_id, java_args.as_ptr())
        };

        // Convert the result into a value
        if self.jvm.has_exception() {
            Err(Error::from_exception(self.jvm))
        } else {
            Ok(Value::from_jvalue(result, &return_type, self.jvm))
        }
    }

    /// Get the value of a public field on this object.
    pub fn field(&self, name: &str, kind: Type) -> Result<Value> {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.jvm.env);

        // Get the field ID and check it exists
        let class = self.class();
        let field_id = class.field_id(name, &kind);
        if field_id == 0 as ffi::jfieldID {
            return Err(Error::from_exception(self.jvm));
        }

        // Get the contents of the field
        let result = unsafe {
            let base: *const ffi::GetFieldFn = mem::transmute(&(**env).GetObjectField);
            let offset = kind.offset();
            let fn_ptr = base.offset(offset as isize);
            (*fn_ptr)(env, self.raw, field_id)
        };

        // Convert the result into a value
        if self.jvm.has_exception() {
            Err(Error::from_exception(self.jvm))
        } else {
            Ok(Value::from_jvalue(result, &kind, self.jvm))
        }
    }

    /// Set the value of a public field on this object.
    pub fn set_field(&self, name: &str, value: Value) -> Result<()> {
        let env: *mut ffi::JNIEnv = ffi::into_raw(self.jvm.env);

        // Get the field ID and check it exists
        let class = self.class();
        let field_id = class.field_id(name, &value);
        if field_id == 0 as ffi::jfieldID {
            return Err(Error::from_exception(self.jvm));
        }

        // Convert the value into a useable form
        let java_value = value.to_jvalue(self.jvm);

        // Set the contents of the field
        unsafe {
            let base: *const ffi::SetFieldFn = mem::transmute(&(**env).SetObjectField);
            let offset = value.offset();
            let fn_ptr = base.offset(offset as isize);
            (*fn_ptr)(env, self.raw, field_id, java_value);
        }

        // Convert the result into a value
        if self.jvm.has_exception() {
            Err(Error::from_exception(self.jvm))
        } else {
            Ok(())
        }
    }
}

//
//  Values and Types
//

/// Implemented by both `Type` and `Value`.
trait Signature {
    /// Returns the identifying type signature for this value.
    fn signature(&self) -> String;
}

/// The type of a Java value returned from a method.
#[derive(Debug, Clone)]
pub enum Type {
    Boolean,
    Byte,
    Char,
    Short,
    Int,
    Long,
    Float,
    Double,
    Str,
    Void,

    /// The argument specifies the fully qualified class name of the object,
    /// eg. `java/lang/String`, using `/` to separate packages.
    ///
    /// This should be known at compile time, hence the static lifetime on the
    /// string.
    Object(&'static str),
}

impl Type {
    /// Returns the function signature component for this value.
    fn static_signature(&self) -> &'static str {
        match self {
            &Type::Boolean => "Z",
            &Type::Byte => "B",
            &Type::Char => "C",
            &Type::Short => "S",
            &Type::Int => "I",
            &Type::Long => "J",
            &Type::Float => "F",
            &Type::Double => "D",
            &Type::Str => "Ljava/lang/String;",
            &Type::Void => "V",
            // The object type is handled properly in the calling function
            &Type::Object(_) => "L",
        }
    }

    /// Returns the integer offset of the corresponding method call function
    /// within the JNIEnv struct.
    fn offset(&self) -> usize {
        match self {
            // Use the `CallObjectMethod` for both objects and strings
            &Type::Object(_) => 0,
            &Type::Str => 0,
            &Type::Boolean => 1,
            &Type::Byte => 2,
            &Type::Char => 3,
            &Type::Short => 4,
            &Type::Int => 5,
            &Type::Long => 6,
            &Type::Float => 7,
            &Type::Double => 8,
            &Type::Void => 9,
        }
    }
}

impl Signature for Type {
    fn signature(&self) -> String {
        let mut result = String::from(self.static_signature());
        if let &Type::Object(class_name_slash) = self {
            // special case for array
            if class_name_slash.starts_with("[") {
                return String::from(class_name_slash);
            }
            result.push_str(class_name_slash);
            result.push(';');
        }
        result
    }
}

/// Expands a `Value` type into one of its subtypes.
macro_rules! expand {
	($name:ident, $enum_name:ident, $kind:ty) => {
		fn $name(self) -> $kind {
			if let Value::$enum_name(value) = self {
				value
			} else {
				panic!("Cannot convert value (`{:?}`) to {}", self, stringify!($kind));
			}
		}
	};
}

/// A value passed to a method call.
///
/// The value has a prescribed lifetime, since it cannot outlive the JVM that
/// created it.
#[derive(Debug)]
pub enum Value<'a> {
    Boolean(bool),
    Byte(i8),
    Char(char),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Str(String),
    Object(Object<'a>),
    Void,
}

impl<'a> Value<'a> {
    /// Returns the function signature component for this value.
    fn static_signature(&self) -> &'static str {
        match self {
            &Value::Boolean(_) => "Z",
            &Value::Byte(_) => "B",
            &Value::Char(_) => "C",
            &Value::Short(_) => "S",
            &Value::Int(_) => "I",
            &Value::Long(_) => "J",
            &Value::Float(_) => "F",
            &Value::Double(_) => "D",
            &Value::Str(_) => "Ljava/lang/String;",
            &Value::Void => "V",
            // The object type is handled properly in the calling function
            &Value::Object(_) => "L",
        }
    }

    /// Returns the integer offset of the corresponding method call function
    /// within the JNIEnv struct.
    fn offset(&self) -> usize {
        match self {
            // Use the `CallObjectMethod` for both objects and strings
            &Value::Object(_) => 0,
            &Value::Str(_) => 0,
            &Value::Boolean(_) => 1,
            &Value::Byte(_) => 2,
            &Value::Char(_) => 3,
            &Value::Short(_) => 4,
            &Value::Int(_) => 5,
            &Value::Long(_) => 6,
            &Value::Float(_) => 7,
            &Value::Double(_) => 8,
            &Value::Void => 9,
        }
    }

    /// Converts the value into a Java value suitable to pass as an argument to
    /// an FFI call.
    fn to_jvalue(&self, jvm: &JavaVM) -> ffi::jvalue {
        let data = unsafe {
            match self {
                &Value::Boolean(v) => mem::transmute(v as u64),
                &Value::Byte(v) => mem::transmute(v as u64),
                &Value::Char(v) => mem::transmute(v as u64),
                &Value::Short(v) => mem::transmute(v as u64),
                &Value::Int(v) => mem::transmute(v as u64),
                &Value::Long(v) => mem::transmute(v as u64),
                &Value::Float(v) => mem::transmute(v as u64),
                &Value::Double(v) => mem::transmute(v as u64),
                &Value::Object(ref v) => mem::transmute(v.raw as u64),
                // TODO: Don't panic, return an exception
                &Value::Void => panic!("Can't pass void to a function"),
                &Value::Str(ref v) => {
                    // TODO: Possible memory leak? Where do we dealloc this?
                    // Does the GC do it for us? I assume so...
                    let env: *mut ffi::JNIEnv = ffi::into_raw(jvm.env);
                    let cstr = CString::new(v.clone()).unwrap();
                    let java_str = ((**env).NewStringUTF)(env, cstr.as_ptr());
                    mem::transmute(java_str as u64)
                }
            }
        };

        ffi::jvalue {
            data: data,
        }
    }

    /// Converts a Java value into its equivalent Rust version.
    fn from_jvalue<'b>(value: ffi::jvalue, kind: &Type, jvm: &'b JavaVM) -> Value<'b> {
        // Depending on the type of the jvalue
        match kind {
            &Type::Boolean => Value::Boolean(value.z() == ffi::JNI_TRUE),
            &Type::Byte => Value::Byte(value.b() as i8),
            &Type::Char => Value::Char(unsafe { char::from_u32_unchecked(value.c() as u32) }),
            &Type::Short => Value::Short(value.s()),
            &Type::Int => Value::Int(value.i()),
            &Type::Long => Value::Long(value.j()),
            &Type::Float => Value::Float(value.f()),
            &Type::Double => Value::Double(value.d()),
            &Type::Void => Value::Void,
            &Type::Object(_) => {
                Value::Object(Object {
                    jvm: jvm,
                    raw: value.l(),
                })
            }
            &Type::Str => {
                // Allocate a new string object and read from the Java string
                let mut result = String::new();
                convert_string(jvm, value.l() as ffi::jstring, &mut result);
                Value::Str(result)
            }
        }
    }

    expand!(as_bool, Boolean, bool);
    expand!(as_byte, Byte, i8);
    expand!(as_char, Char, char);
    expand!(as_short, Short, i16);
    expand!(as_int, Int, i32);
    expand!(as_long, Long, i64);
    expand!(as_float, Float, f32);
    expand!(as_double, Double, f64);
    expand!(as_object, Object, Object<'a>);
    expand!(as_str, Str, String);
}

impl<'a> Signature for Value<'a> {
    fn signature(&self) -> String {
        let mut result = String::from(self.static_signature());
        if let &Value::Object(ref obj) = self {
            let class_name_slash = obj.class_name_slash().unwrap();
            // special case for array
            if class_name_slash.starts_with("[") {
                return class_name_slash;
            }
            result.push_str(class_name_slash.as_str());
        }
        result
    }
}

/// Returns the function signature as a string for a method with the given
/// arguments and return type.
fn function_signature(args: &[Value], return_type: &Type) -> String {
    let mut sig = String::new();

    // Push the opening bracket for the arguments type list
    sig.push('(');

    // Iterate over each argument
    for arg in args {
        // Each Java type has a 1 character type associated with it, which we
        // push onto the signature to indicate another argument to the function
        sig.push_str(arg.signature().as_str());
    }

    // Push the closing bracket to the arguments list
    sig.push(')');

    // Push the return type's signature
    sig.push_str(return_type.signature().as_str());

    sig
}

/// Convert the given Java string into the proper Rust version, and push it onto
/// the given String.
fn convert_string(jvm: &JavaVM, java_str: ffi::jstring, result: &mut String) {
    let env: *mut ffi::JNIEnv = ffi::into_raw(jvm.env);

    // Convert the JNI string to something we can actually use
    let name = unsafe {
        let ptr = ((**env).GetStringUTFChars)(env, java_str, ptr::null_mut());
        CStr::from_ptr(ptr)
    };

    // Copy the UTF-8 converted string into a new, heap allocated one
    let utf8 = name.to_str().unwrap();
    result.push_str(utf8);

    // Free the java string
    unsafe {
        ((**env).ReleaseStringUTFChars)(env, java_str, name.as_ptr());
    }
}

//
//  Error Handling
//

/// A result type that wraps the JVM initialisation error.
pub type Result<T> = std::result::Result<T, Error>;

/// An error returned when creating an instance of a JVM.
pub enum Error {
    /// An unsupported version error.
    UnsupportedVersion,

    /// An out of memory error.
    OutOfMemory,

    /// An internal FFI error.
    FFIError(ffi::JNIError),

    /// An exception raised in Java code.
    Exception(ExceptionInfo),
}

impl Error {
    /// Create a new error from an underlying FFI code.
    fn from_ffi(code: ffi::JNIError) -> Error {
        match code {
            ffi::JNIError::JNI_EVERSION => Error::UnsupportedVersion,
            ffi::JNIError::JNI_ENOMEM => Error::OutOfMemory,
            _ => Error::FFIError(code),
        }
    }

    /// Create a new error from the most recent exception. The caller guarantees
    /// that an exception has occurred.
    fn from_exception(jvm: &JavaVM) -> Error {
        // Get the thrown exception
        let obj = jvm.exception_obj();
        jvm.clear_exception();

        // Class name
        let class_name = obj.class_name().unwrap();

        // Get the message associated with the error
        let msg = obj.call("toString", &[], Type::Str).unwrap().as_str();

        // Create the exception object
        Error::Exception(ExceptionInfo {
            name: class_name,
            message: msg,
        })
    }
}

impl error::Error for Error {
    fn description<'a>(&'a self) -> &'a str {
        match self {
            &Error::UnsupportedVersion => "Unsupported JVM version",
            &Error::OutOfMemory => "Out of memory",
            &Error::Exception(ref info) => info.message(),
            &Error::FFIError(code) => {
                match code {
                    ffi::JNIError::JNI_OK => "Success?",
                    ffi::JNIError::JNI_ERR => "Unknown error",
                    ffi::JNIError::JNI_EDETACHED => "Thread detached from JVM",
                    ffi::JNIError::JNI_EVERSION => "Unsupported JVM version",
                    ffi::JNIError::JNI_ENOMEM => "Out of memory",
                    ffi::JNIError::JNI_EEXIST => "JVM has already been created",
                    ffi::JNIError::JNI_EINVAL => "Invalid arguments to JNI function",
                }
            }
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::Exception(ref info) => info.fmt(f),
            _ => {
                use std::error::Error;
                write!(f, "{}", self.description())
            }
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self) }
}

/// Information associated with an exception.
pub struct ExceptionInfo {
    name: String,
    message: String,
}

impl ExceptionInfo {
    /// Returns the class name of the exception.
    pub fn name<'a>(&'a self) -> &'a str { &self.name }

    /// Returns the detailed error message associated with the exception.
    pub fn message<'a>(&'a self) -> &'a str { &self.message }
}

impl fmt::Display for ExceptionInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { writeln!(f, "[ERROR] {}", self.message) }
}

impl fmt::Debug for ExceptionInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self) }
}
