module examples.platform

@target(.macos) {
    module examples.platform.darwin

    extern "C" {
        fn getpid() -> Int32
    }

    fn page_size() -> Int {
        darwin.page_size()
    }
}

@target(all(.macos, .aarch64)) {
    fn arch_name() -> String {
        "apple-silicon"
    }
}

@target(any(.aarch64, .x86_64)) {
    fn word_bits() -> Int {
        64
    }
}

@target(not(.windows)) {
    fn path_separator() -> String {
        "/"
    }
}
