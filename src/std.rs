pub const PRELUDE_VALUES: &[&str] = &[
    "assert", "print", "io", "fs", "http", "sse", "time", "thread", "async", "channel", "sync",
    "atomic", "task", "box", "vec", "shared", "darwin",
];

pub const CORE_TYPES: &[&str] = &[
    "Bool",
    "Byte",
    "Int",
    "Int8",
    "Int16",
    "Int32",
    "Int64",
    "UInt",
    "UInt8",
    "UInt16",
    "UInt32",
    "UInt64",
    "Float32",
    "Float64",
    "Char",
    "Unit",
    "Never",
    "String",
    "Str",
    "Slice",
    "Array",
    "Option",
    "Result",
    "Vec",
    "Map",
    "Set",
    "Box",
    "Shared",
    "Error",
    "IOError",
    "Request",
    "Response",
    "Server",
    "JoinHandle",
    "Mutex",
    "RwLock",
    "Channel",
    "Sender",
    "Receiver",
    "IntChannel",
    "AtomicInt",
    "ThreadPool4",
];

pub const CORE_INTERFACES: &[&str] = &[
    "Copy", "Clone", "Drop", "Display", "Debug", "Eq", "Ord", "Hash", "Send", "Sync",
];

pub fn prelude_values() -> impl Iterator<Item = &'static str> {
    PRELUDE_VALUES.iter().copied()
}

pub fn core_types() -> impl Iterator<Item = &'static str> {
    CORE_TYPES.iter().chain(CORE_INTERFACES).copied()
}
