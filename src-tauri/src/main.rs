// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    unsafe {
        use std::os::raw::{c_int, c_void};
        // Link to X11 library
        #[link(name = "X11")]
        extern "C" {
            fn XInitThreads() -> c_int;
        }
        // Initialize X11 threads support to prevent crash with screenshots crate
        XInitThreads();
    }

    lumen_lib::run()
}
