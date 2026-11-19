//! Graphics card detection helpers.

use shirtbuilder::gpu::{nvidia_versions, version_in};

#[test]
fn reads_driver_versions() {
    let proc_line = "NVRM version: NVIDIA UNIX Open Kernel Module for x86_64  615.78.08  Release Build  (notroot@)  Fri Oct 23 16:34:25 UTC 2026";
    assert_eq!(version_in(proc_line.split_once("Kernel Module").unwrap().1).as_deref(), Some("615.78.08"));
    assert_eq!(version_in("for x86_64  550.127  Release").as_deref(), Some("550.127"));
    assert_eq!(version_in("no version here"), None);
}

#[test]
fn nvidia_versions_on_this_machine() {
    // informative only: on a machine with the NVIDIA driver this prints kernel vs. installed
    if let Some((kernel, user)) = nvidia_versions() {
        println!("NVIDIA kernel module {kernel}, libraries {user}");
        assert!(kernel.contains('.') && user.contains('.'));
    }
}
