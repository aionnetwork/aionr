#[cfg(debug_assertions)]
/// Roughly estimate what stack size each level of evm depth will use. (Debug build)
pub const STACK_SIZE_PER_DEPTH: usize = 128 * 1024;

#[cfg(not(debug_assertions))]
/// Roughly estimate what stack size each level of evm depth will use.
pub const STACK_SIZE_PER_DEPTH: usize = 128 * 1024;

#[cfg(debug_assertions)]
// /// Entry stack overhead prior to execution. (Debug build)
pub const STACK_SIZE_ENTRY_OVERHEAD: usize = 100 * 1024;

#[cfg(not(debug_assertions))]
/// Entry stack overhead prior to execution.
pub const STACK_SIZE_ENTRY_OVERHEAD: usize = 20 * 1024;
