#[cfg(target_os = "windows")]
extern crate winapi;

#[cfg(target_os = "windows")]
use winapi::um::processthreadsapi::GetCurrentProcessId;
use winapi::um::processthreadsapi::GetCurrentThreadId;

#[cfg(target_os = "linux")]
extern crate libc;

#[cfg(target_os = "linux")]
use libc::getpid;


// GetCurrentProcessID
#[no_mangle]
pub extern "C" fn get_current_process_id() -> u32 {
    #[cfg(target_os = "windows")]
    {
        unsafe { GetCurrentProcessId() }
    }
    #[cfg(target_os = "linux")]
    {
        unsafe { getpid() as u32 }
    }
}
// GetCurrentThreadID
#[no_mangle]
pub extern "C" fn get_current_thread_id() -> u32 {
    #[cfg(target_os = "windows")]
    {
        unsafe { GetCurrentThreadId() }
    }
    #[cfg(target_os = "linux")]
    {
        unsafe { libc::pthread_self() as u32 }
    }
}
// GetProcessAndThreadIDStr
#[no_mangle]
pub extern "C" fn GetProcessAndThreadIDStr() -> String {
    let pid = get_current_process_id();
    let tid = get_current_thread_id();
    format!("ptid{}", pid+tid)
}
