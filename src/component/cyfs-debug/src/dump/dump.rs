use std::path::Path;

#[cfg(windows)]
mod dump_win {
    use std::ffi::c_void;
    use std::os::windows::prelude::*;
    use std::os::windows::raw::HANDLE;
    use std::path::Path;
    use winapi::shared::minwindef::{BOOL, DWORD};
    use winapi::shared::ntdef::NULL;
    use winapi::um::processthreadsapi::{GetCurrentProcess, GetCurrentProcessId};

    #[repr(C)]
    enum MINIDUMP_TYPE {
        MiniDumpNormal = 0x00000000,
        MiniDumpWithDataSegs = 0x00000001,
        MiniDumpWithFullMemory = 0x00000002,
        MiniDumpWithHandleData = 0x00000004,
    }

    #[link(name = "Dbghelp")]
    extern "system" {
        fn MiniDumpWriteDump(
            hProcess: HANDLE,
            ProcessId: DWORD,
            hFile: HANDLE,
            DumpType: MINIDUMP_TYPE,
            ExceptionParam: *mut c_void,
            UserStreamParam: *mut c_void,
            CallbackParam: *mut c_void,
        ) -> BOOL;
    }

    pub unsafe fn create_dump(dir: &Path, filename: &str, full_dump: bool) {
        let pid = GetCurrentProcessId();
        let new_name = filename.replace("%p", &pid.to_string());
        let dump_path = dir.join(&new_name);
        let file = std::fs::File::create(&dump_path).unwrap();

        let dump_type = if full_dump {
            MINIDUMP_TYPE::MiniDumpWithFullMemory
        } else {
            MINIDUMP_TYPE::MiniDumpNormal
        };

        let ret = MiniDumpWriteDump(
            GetCurrentProcess(),
            GetCurrentProcessId(),
            file.as_raw_handle(),
            dump_type,
            NULL,
            NULL,
            NULL,
        );
        if ret == 0 {
            error!("create dump failed! ret={}", ret);
        }

        std::process::exit(-1);
    }
}

#[cfg(not(windows))]
mod dump_posix {
    use libc::{getpid, kill, SIGABRT};
    use std::path::Path;

    pub unsafe fn create_dump(_dir: &Path, _filename: &str, _full_dump: bool) {
        let ret = kill(getpid(), SIGABRT);
        println!("linux send SIGABRT ret {}", ret);
    }
}

pub fn create_dump(dir: &Path, filename: &str, full_dump: bool) {
    #[cfg(windows)]
    unsafe {
        dump_win::create_dump(dir, filename, full_dump);
    }

    #[cfg(not(windows))]
    unsafe {
        dump_posix::create_dump(dir, filename, full_dump);
    }
}
