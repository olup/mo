/// Shared ownership/resource classification used by drop checking, IR lowering,
/// and backend codegen.
///
/// This is intentionally conservative while Mo's public ownership model is still
/// being generalized. Unique resources may run destructive automatic cleanup at
/// scope exit. Shared-handle resources are cloneable handles to shared internals;
/// they must not be destroyed by an ordinary local drop until the type has a
/// distinct unique owner or reference-counted inner resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceOwnership {
    UniqueOwner,
    SharedHandle,
    Ordinary,
}

pub fn classify_named_type(name: &str) -> ResourceOwnership {
    let name = unqualified_name(name);
    if is_shared_handle_name(name) {
        ResourceOwnership::SharedHandle
    } else if is_unique_resource_name(name) {
        ResourceOwnership::UniqueOwner
    } else {
        ResourceOwnership::Ordinary
    }
}

pub fn is_unique_resource_name(name: &str) -> bool {
    matches!(
        unqualified_name(name),
        "Buffer"
            | "ByteBuffer"
            | "Box"
            | "Map"
            | "Shared"
            | "StringBuilder"
            | "Vec"
            | "TaskQueue4"
            | "TaskQueue4Int"
            | "TcpListener"
            | "TcpStream"
            | "box__Box"
            | "buffer__ByteBuffer"
            | "buffer__Buffer"
            | "buffer__StringBuilder"
            | "map__Map"
            | "shared__Shared"
            | "vec__Vec"
            | "task__TaskQueue4"
            | "task__TaskQueue4Int"
            | "net__TcpListener"
            | "net__TcpStream"
    )
}

pub fn is_shared_handle_name(name: &str) -> bool {
    matches!(unqualified_name(name), "Channel" | "channel__Channel")
}

fn unqualified_name(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}
