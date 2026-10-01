use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, MAX_PATH};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};

pub struct ForegroundWindow {
    pub title: String,
    pub process_name: String,
}

/// The currently-focused window's title and owning process's executable
/// name (e.g. "notepad.exe"), used both by the Privacy Guard and to key
/// per-app conversation history. Returns None if there is no foreground
/// window or its process info can't be read.
pub fn foreground_window() -> Option<ForegroundWindow> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }

        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
        let title = String::from_utf16_lossy(&title_buf[..len.max(0) as usize]);

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return Some(ForegroundWindow {
                title,
                process_name: String::new(),
            });
        }

        let process_handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process_handle.is_null() {
            return Some(ForegroundWindow {
                title,
                process_name: String::new(),
            });
        }

        let mut path_buf = [0u16; MAX_PATH as usize];
        let mut size = path_buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            process_handle,
            PROCESS_NAME_WIN32,
            path_buf.as_mut_ptr(),
            &mut size,
        );
        CloseHandle(process_handle);

        let process_name = if ok != 0 {
            String::from_utf16_lossy(&path_buf[..size as usize])
                .rsplit('\\')
                .next()
                .unwrap_or_default()
                .to_lowercase()
        } else {
            String::new()
        };

        Some(ForegroundWindow { title, process_name })
    }
}
