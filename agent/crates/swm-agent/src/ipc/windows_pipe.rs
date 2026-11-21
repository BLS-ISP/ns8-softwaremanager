#[cfg(windows)]
pub mod imp {
    use crate::ipc::{handle_ipc_stream, SharedAgentIpcContext};
    use anyhow::{Context, Result};
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
    use tokio::sync::watch;

    #[repr(C)]
    struct SecurityAttributes {
        n_length: u32,
        lp_security_descriptor: *mut std::ffi::c_void,
        b_inherit_handle: i32,
    }

    #[link(name = "advapi32")]
    extern "system" {
        fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
            string_security_descriptor: *const u16,
            string_sd_revision: u32,
            security_descriptor: *mut *mut std::ffi::c_void,
            security_descriptor_size: *mut u32,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(h_mem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    fn create_pipe_instance(pipe_name: &str, is_first: bool) -> std::io::Result<NamedPipeServer> {
        let mut opts = ServerOptions::new();
        opts.first_pipe_instance(is_first)
            .reject_remote_clients(true)
            .in_buffer_size(65536)
            .out_buffer_size(65536);

        // SDDL: SYSTEM (GA), Administrators (GA), Authenticated Users (GRGW), Interactive Users (GRGW)
        let sddl = "D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;AU)(A;;GRGW;;;IU)";
        let wide: Vec<u16> = OsStr::new(sddl)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut sd_ptr: *mut std::ffi::c_void = ptr::null_mut();
        // SDDL_REVISION_1 = 1
        let ok = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide.as_ptr(),
                1,
                &mut sd_ptr,
                ptr::null_mut(),
            )
        };

        if ok != 0 && !sd_ptr.is_null() {
            let mut sa = SecurityAttributes {
                n_length: std::mem::size_of::<SecurityAttributes>() as u32,
                lp_security_descriptor: sd_ptr,
                b_inherit_handle: 0,
            };
            let res = unsafe {
                opts.create_with_security_attributes_raw(
                    pipe_name,
                    (&mut sa as *mut SecurityAttributes).cast(),
                )
            };
            unsafe {
                LocalFree(sd_ptr);
            }
            if res.is_ok() {
                return res;
            }
        }

        opts.create(pipe_name)
    }

    pub async fn serve_windows_named_pipe(
        pipe_name: String,
        ctx: SharedAgentIpcContext,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> Result<()> {
        let mut server = create_pipe_instance(&pipe_name, true)
            .with_context(|| format!("Failed creating Windows Named Pipe '{pipe_name}'"))?;

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
                conn_res = server.connect() => {
                    conn_res.with_context(|| format!("Named Pipe connect failed on '{pipe_name}'"))?;
                    let connected_client = server;
                    server = create_pipe_instance(&pipe_name, false)
                        .with_context(|| format!("Failed creating next Named Pipe instance on '{pipe_name}'"))?;
                    let client_ctx = ctx.clone();
                    tokio::spawn(async move {
                        let _ = handle_ipc_stream(connected_client, client_ctx).await;
                    });
                }
            }
        }
        Ok(())
    }
}
