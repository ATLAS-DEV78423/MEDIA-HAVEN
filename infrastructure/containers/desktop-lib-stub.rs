// Container-build stub. Dockerfile.core copies this to
// apps/desktop/src-tauri/src/lib.rs so Cargo can load the workspace member
// manifest. The desktop crate is never built in the core image; only its
// manifest and lib target path must resolve. The real source lives in
// apps/desktop/src-tauri/src/lib.rs and is excluded from the build context.
