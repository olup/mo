use crate::semantics::Target;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub triple: String,
    pub libc_symbols: Vec<String>,
    pub thread_symbols: Vec<String>,
    pub socket_symbols: Vec<String>,
    pub time_symbols: Vec<String>,
    pub allocator_symbols: Vec<String>,
}

pub fn runtime_for_target(target: &Target) -> RuntimeConfig {
    if target.has("macos") && target.has("aarch64") {
        return RuntimeConfig {
            triple: "aarch64-apple-darwin".to_string(),
            libc_symbols: strings(&["getpid", "puts", "strlen", "close", "read", "write"]),
            thread_symbols: strings(&[
                "pthread_create",
                "pthread_join",
                "pthread_self",
                "pthread_mutex_init",
                "pthread_mutex_lock",
                "pthread_mutex_unlock",
                "pthread_mutex_destroy",
                "pthread_rwlock_init",
                "pthread_rwlock_rdlock",
                "pthread_rwlock_wrlock",
                "pthread_rwlock_unlock",
                "pthread_rwlock_destroy",
                "pthread_cond_init",
                "pthread_cond_wait",
                "pthread_cond_signal",
                "pthread_cond_broadcast",
                "pthread_cond_destroy",
            ]),
            socket_symbols: strings(&[
                "socket", "bind", "listen", "accept", "connect", "send", "recv",
            ]),
            time_symbols: strings(&["mach_absolute_time", "clock_gettime", "nanosleep"]),
            allocator_symbols: strings(&["malloc", "calloc", "realloc", "free"]),
        };
    }

    if target.has("linux") {
        let triple = if target.has("aarch64") {
            "aarch64-unknown-linux-gnu"
        } else if target.has("x86_64") {
            "x86_64-unknown-linux-gnu"
        } else {
            "unknown-linux-gnu"
        };
        return RuntimeConfig {
            triple: triple.to_string(),
            libc_symbols: strings(&["getpid", "puts", "strlen", "close", "read", "write"]),
            thread_symbols: strings(&[
                "pthread_create",
                "pthread_join",
                "pthread_self",
                "pthread_mutex_init",
                "pthread_mutex_lock",
                "pthread_mutex_unlock",
                "pthread_mutex_destroy",
                "pthread_rwlock_init",
                "pthread_rwlock_rdlock",
                "pthread_rwlock_wrlock",
                "pthread_rwlock_unlock",
                "pthread_rwlock_destroy",
                "pthread_cond_init",
                "pthread_cond_wait",
                "pthread_cond_signal",
                "pthread_cond_broadcast",
                "pthread_cond_destroy",
            ]),
            socket_symbols: strings(&[
                "socket", "bind", "listen", "accept", "connect", "send", "recv",
            ]),
            time_symbols: strings(&["clock_gettime", "nanosleep"]),
            allocator_symbols: strings(&["malloc", "calloc", "realloc", "free"]),
        };
    }

    RuntimeConfig {
        triple: "unknown".to_string(),
        libc_symbols: Vec::new(),
        thread_symbols: Vec::new(),
        socket_symbols: Vec::new(),
        time_symbols: Vec::new(),
        allocator_symbols: Vec::new(),
    }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}
