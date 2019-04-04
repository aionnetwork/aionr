use callback::register_callbacks;
use codec::NativeDecoder;
use codec::NativeEncoder;
use rjni::{Classpath, JavaVM, Options, Type, Value, Version};
use rjni::ffi;
use std::fs;
use std::io::Error;
use std::path::Path;
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::sync::mpsc::channel;
use types::{TransactionContext, TransactionResult};
use vm_common::{EnvInfo, CallType};
use bytes::Bytes;
use aion_types::{Address, U256, H256};
use hash::{BLAKE2B_EMPTY};


/// We keep a single JVM instance in the background, which will be shared
/// among multiple threads. Before invoking any JNI methods, the executing
/// thread needs to attach the thread to the JVM instance first and deattach
/// after finishing the interaction.
static mut JVM_SINGLETON: AtomicPtr<ffi::JavaVM> = AtomicPtr::new(ptr::null_mut());
const AVM_JARS: [&str; 17] = [
    "asm-6.2.1.jar",
    "asm-analysis-6.2.1.jar",
    "asm-commons-6.2.1.jar",
    "asm-tree-6.2.1.jar",
    "asm-util-6.2.1.jar",
    "ed25519.jar",
    "hamcrest-all-1.3.jar",
    "org-aion-avm-api.jar",
    "org-aion-avm-core.jar",
    "org-aion-avm-rt.jar",
    "org-aion-avm-tooling.jar",
    "org-aion-avm-userlib.jar",
    "scratch-deps.jar",
    "slf4j-api-1.7.25.jar",
    "spongycastle-1.58.0.0.jar",
    "vm-api-e8657a6.jar",
    "org-aion-avm-jni.jar",
];

/// Creates a JVM instance for the first time. This method is NOT thread
/// safe, and is intended for the "main" thread only.
pub fn launch_jvm() {
    unsafe {
        if ptr::eq(JVM_SINGLETON.load(Ordering::Relaxed), ptr::null_mut()) {
            let child = thread::spawn(move || {
                // prepare classpath
                let mut classpath = Classpath::new();
                let mut libs = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                libs.push("libs");
                for jar_pkg in AVM_JARS.iter() {
                    let mut pkg_path = libs.clone();
                    pkg_path.push(jar_pkg);
                    println!("add jar {:?}", pkg_path);
                    classpath = add_jars(
                        classpath,
                        pkg_path.to_str().expect("The `libs` folder is not found"),
                    );
                }

                // prepare options
                let mut options = Options::new();
                options = options.version(Version::V18);
                options = options.classpath(classpath);

                // launch jvm
                let jvm = JavaVM::new(options).expect("Failed to launch a JVM instance");

                // register callbacks
                register_callbacks();

                // save the ffi::JavaVM instance
                JVM_SINGLETON.store(ffi::into_raw(jvm.vm), Ordering::Relaxed);
            });

            child
                .join()
                .expect("Couldn't join on the JVM launcher thread");
        }
    }
}

/// Add a jar file to the classpath, or all jars in the folder if the path
/// is a directory.
fn add_jars(cp: Classpath, path: &str) -> Classpath {
    let mut result = cp;

    if path.ends_with(".jar") {
        result = result.add(&Path::new(path));
    } else {
        let r = find_files(&Path::new(path), ".jar");
        match r {
            Ok(files) => {
                for file in files {
                    result = result.add(&Path::new(file.as_str()));
                }
            }
            Err(_) => {}
        }
    }

    result
}

/// Walk through the given path and return a list of files with the specified
/// extension.
fn find_files(path: &Path, extension: &str) -> Result<Vec<String>, Error> {
    let mut result: Vec<String> = vec![];

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let path = entry?.path();
            let mut temp = find_files(&path, extension)?;
            result.append(&mut temp);
        }
    } else {
        let path_str = path.to_str();
        match path_str {
            Some(p) => {
                if p.ends_with(extension) {
                    result.push(String::from(p));
                }
            }
            None => {}
        }
    }

    return Ok(result);
}

/// Aion virtual machine
#[derive(Clone)]
pub struct AVM {
    jvm: JavaVM,
}

impl Drop for AVM {
    fn drop(&mut self) {
        let vm: *mut ffi::JavaVM = ffi::into_raw(self.jvm.vm);
        unsafe {
            ((**vm).DetachCurrentThread)(vm);
        }
    }
}

// static mut ExecutorClass: AtomicPtr<Class> = AtomicPtr::new(ptr::null_mut());

impl AVM {
    /// create a new AVM instance
    pub fn new() -> AVM {
        // launch a JVM if not done so far
        launch_jvm();

        // attach this thread to the JVM
        unsafe {
            let vm = JVM_SINGLETON.load(Ordering::Relaxed);
            let env: *mut ffi::JNIEnv = ptr::null_mut();

            //((**vm).AttachCurrentThread)(vm, &mut env, ptr::null_mut());

            AVM {
                jvm: JavaVM {
                    vm: ffi::from_raw(vm),
                    env: ffi::from_raw(env),
                },
            }
        }
    }

    fn attach(&self) -> Self {
        unsafe {
            let vm = JVM_SINGLETON.load(Ordering::Relaxed);
            let mut env = ptr::null_mut();
            ((**vm).AttachCurrentThread)(vm, &mut env, ptr::null_mut());

            AVM {
                jvm: JavaVM {
                    vm: self.jvm.vm,
                    env: ffi::from_raw(env),
                },
            }
        }
    }

    fn dettach(&self) {
        let vm: *mut ffi::JavaVM = ffi::into_raw(self.jvm.vm);
        unsafe {
            ((**vm).DetachCurrentThread)(vm);
        }
    }

    /// Executes a list of transactions
    pub fn execute(
        &self,
        ext_hdl: i64,
        transactions: &Vec<TransactionContext>,
    ) -> Result<Vec<TransactionResult>, &'static str>
    {
        println!("start rust jvm executor");
        let vm = self.attach();
        // find the NativeTransactionExecutor class
        let class = vm
            .jvm
            .class("org/aion/avm/jni/NativeTransactionExecutor")
            .expect("NativeTransactionExecutor is missing in the classpath");

        println!("load native class");
        // the method name
        let name = "execute";

        // the method return type
        let return_type = Type::Object("[B");

        // the arguments
        let arguments = [
            Value::Long(ext_hdl), // handle
            Value::Object(
                vm.jvm
                    .new_byte_array_with_data(&Self::encode_transaction_contexts(&transactions))
                    .expect("Failed to create new byte array in JVM"),
            ),
        ];

        println!("rust jvm call_static");
        // invoke the method
        let ret = class
            .call_static(name, &arguments, return_type)
            .expect("Failed to call the execute() method");

        if let Value::Object(obj) = ret {
            if obj.is_null() {
                Err("The execute() method failed")
            } else {
                let bytes = vm.jvm.load_byte_array(&obj);
                self.dettach();
                Self::decode_transaction_results(&bytes)
            }
        } else {
            Err("The execute() method returns wrong data")
        }
    }

    /// Encodes transaction contexts into byte array
    fn encode_transaction_contexts(transactions: &Vec<TransactionContext>) -> Vec<u8> {
        let mut encoder = NativeEncoder::new();
        encoder.encode_int(transactions.len() as u32);
        for i in 0..transactions.len() {
            encoder.encode_bytes(&transactions[i].to_bytes());
        }

        encoder.to_bytes()
    }

    /// Decodes transaction results from byte array
    fn decode_transaction_results(bytes: &Vec<u8>) -> Result<Vec<TransactionResult>, &'static str> {
        let mut results = Vec::<TransactionResult>::new();
        let mut decoder = NativeDecoder::new(bytes);
        let length = decoder.decode_int()?;
        for _i in 0..length {
            let result = decoder.decode_bytes()?;
            let state_root = decoder.decode_bytes()?;
            results.push(TransactionResult::new(result, state_root)?);
        }

        Ok(results)
    }
}

pub trait AVMExt {
    fn create_account(&mut self, address: &Address);
    fn account_exists(&self, address: &Address) -> bool;
    fn save_code(&mut self, address: &Address, code: Vec<u8>);
    fn get_code(&self, address: &Address) -> Option<Arc<Vec<u8>>>;
    fn sstore(&mut self, address: &Address, key: Vec<u8>, value: Vec<u8>);
    fn sload(&self, address: &Address, key: &Vec<u8>) -> Option<Vec<u8>>;
    fn remove_account(&mut self, address: &Address);
    fn avm_balance(&self, address: &Address) -> U256;
    fn inc_balance(&mut self, address: &Address, inc: &U256);
    fn dec_balance(&mut self, address: &Address, dec: &U256);
    fn get_nonce(&self, address: &Address) -> u64;
    fn inc_nonce(&mut self, address: &Address);
    fn env_info(&self) -> &EnvInfo;
    fn depth(&self) -> usize;
    fn touch_account(&mut self, address: &Address, index: i32);
    fn send_signal(&mut self, signal: i32);
    fn commit(&mut self);
    fn root(&self) -> H256;
}

// TODO: should be a trait, possible to avoid cloning everything from a Transaction(/View).
/// Action (call/create) input params. Everything else should be specified in Externalities.
#[derive(Clone, Debug)]
pub struct AVMActionParams {
    /// Address of currently executed code.
    pub code_address: Address,
    /// Hash of currently executed code.
    pub code_hash: Option<H256>,
    /// Receive address. Usually equal to code_address,
    /// except when called using CALLCODE.
    pub address: Address,
    /// Sender of current part of the transaction.
    pub sender: Address,
    /// Transaction initiator.
    pub origin: Address,
    /// Gas paid up front for transaction execution
    pub gas: U256,
    /// Gas price.
    pub gas_price: U256,
    /// Transaction value.
    pub value: U256,
    /// Code being executed.
    pub code: Option<Arc<Bytes>>,
    /// Input data.
    pub data: Option<Bytes>,
    /// Type of call
    pub call_type: CallType,
    /// transaction hash
    pub transaction_hash: H256,
    /// original transaction hash
    pub original_transaction_hash: H256,
    /// Nonce
    pub nonce: u64,
}

impl Default for AVMActionParams {
    /// Returns default ActionParams initialized with zeros
    fn default() -> AVMActionParams {
        AVMActionParams {
            code_address: Address::new(),
            code_hash: Some(BLAKE2B_EMPTY),
            address: Address::new(),
            sender: Address::new(),
            origin: Address::new(),
            gas: U256::zero(),
            gas_price: U256::zero(),
            value: U256::zero(),
            code: None,
            data: None,
            call_type: CallType::None,
            transaction_hash: H256::default(),
            original_transaction_hash: H256::default(),
            nonce: 0,
        }
    }
}

#[cfg(test)]
mod test {
    use avm::AVM;
    use codec::NativeEncoder;
    use std::fs::File;
    use std::io::Error;
    use std::io::Read;
    use std::path::PathBuf;
    use types::TransactionContext;
    use avm_abi::{AbiToken, AVMEncoder};

    #[test]
    fn test_avm_hello_world() {
        let avm = AVM::new();
        let transactions = prepare_transactions();
        let results = avm.execute(0, &transactions).unwrap();
        println!("{:?}", results);
    }

    fn prepare_transactions() -> Vec<TransactionContext> {
        let mut file = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        file.push("examples/com.example.helloworld.jar");
        let file_str = file.to_str().expect("Failed to locate the helloworld.jar");

        let tx1 = TransactionContext {
            transaction_type: 2,
            address: [1u8; 32].to_vec(),
            caller: [2u8; 32].to_vec(),
            origin: [3u8; 32].to_vec(),
            nonce: 0,
            value: vec![99],
            data: code_and_arguments(
                &read_file(file_str).expect("Failed to read the helloworld.jar"),
                Option::None,
            ),
            energy_limit: 1000_00,
            energy_price: 1,
            transaction_hash: [4u8; 32].to_vec(),
            basic_cost: 200_000,
            transaction_timestamp: 2,
            block_timestamp: 3,
            block_number: 4,
            block_energy_limit: 5_000_000,
            block_coinbase: [4u8; 32].to_vec(),
            block_previous_hash: [5u8; 32].to_vec(),
            block_difficulty: Vec::new(),
            internal_call_depth: 0,
        };

        let tx2 = TransactionContext {
            transaction_type: 2,
            address: [1u8; 32].to_vec(),
            caller: [5u8; 32].to_vec(),
            origin: [6u8; 32].to_vec(),
            nonce: 0,
            value: vec![10],
            data: code_and_arguments(
                &read_file(file_str).expect("Failed to read the helloworld.jar"),
                Option::None,
            ),
            energy_limit: 1_000_000,
            energy_price: 1,
            transaction_hash: [4u8; 32].to_vec(),
            basic_cost: 200_000,
            transaction_timestamp: 2,
            block_timestamp: 3,
            block_number: 4,
            block_energy_limit: 5_000_000,
            block_coinbase: [4u8; 32].to_vec(),
            block_previous_hash: [5u8; 32].to_vec(),
            block_difficulty: Vec::new(),
            internal_call_depth: 0,
        };

        let tx3 = TransactionContext {
            transaction_type: 3,
            address: [1u8; 32].to_vec(),
            caller: [5u8; 32].to_vec(),
            origin: [6u8; 32].to_vec(),
            nonce: 1,
            value: vec![10],
            data: AbiToken::STRING("sayHello".to_string()).encode(),
            energy_limit: 1_000_000,
            energy_price: 1,
            transaction_hash: [4u8; 32].to_vec(),
            basic_cost: 200_000,
            transaction_timestamp: 2,
            block_timestamp: 3,
            block_number: 4,
            block_energy_limit: 5_000_000,
            block_coinbase: [4u8; 32].to_vec(),
            block_previous_hash: [5u8; 32].to_vec(),
            block_difficulty: Vec::new(),
            internal_call_depth: 0,
        };

        let mut tx_contexts = Vec::<TransactionContext>::new();
        tx_contexts.push(tx1);
        tx_contexts.push(tx2);
        tx_contexts.push(tx3);
        tx_contexts
    }

    fn code_and_arguments(code: &Vec<u8>, arguments: Option<&Vec<u8>>) -> Vec<u8> {
        let mut encoder = NativeEncoder::new();
        encoder.encode_bytes(code);
        match arguments {
            Some(arg) => encoder.encode_bytes(arg),
            None => {}
        }
        encoder.to_bytes()
    }

    fn read_file(path: &str) -> Result<Vec<u8>, Error> {
        let mut file = File::open(path)?;
        let mut buf = Vec::<u8>::new();
        file.read_to_end(&mut buf)?;
        Ok(buf)
    }
}
