//! Host-owned top-level Win32 shell for embedded child CLAP editors.

use std::io;

use clack_extensions::gui::GuiSize;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyWindow, IsIconic, IsWindow,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, CW_USEDEFAULT, GWLP_WNDPROC, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOZORDER, SW_HIDE, SW_RESTORE, SW_SHOW, WM_CLOSE, WS_CAPTION, WS_CLIPCHILDREN,
    WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU,
};

const CHILD_WINDOW_STYLE: u32 =
    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;

pub(crate) struct DetachedChildWindow {
    hwnd: HWND,
}

impl DetachedChildWindow {
    pub(crate) fn create(title: &str, client_size: GuiSize) -> io::Result<Self> {
        let class: Vec<u16> = "STATIC".encode_utf16().chain(Some(0)).collect();
        let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
        let (width, height) = outer_size(client_size)?;
        // SAFETY: class and title are terminated UTF-16 strings; the returned HWND is owned here.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                CHILD_WINDOW_STYLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                width,
                height,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        if hwnd.is_null() {
            return Err(io::Error::last_os_error());
        }

        // The predefined STATIC class has control-specific message handling that is unsuitable for
        // a movable top-level plugin shell. Replace it with DefWindowProc semantics so caption drag,
        // minimize and the system menu behave like a normal Win32 tool window. WM_CLOSE is handled
        // specially below: hiding preserves the embedded CLAP parent until gui.destroy().
        // SAFETY: hwnd is a live same-process window and the callback has the Win32 WNDPROC ABI.
        let previous = unsafe {
            SetWindowLongPtrW(
                hwnd,
                GWLP_WNDPROC,
                detached_child_window_proc as usize as isize,
            )
        };
        if previous == 0 {
            let error = io::Error::last_os_error();
            unsafe { DestroyWindow(hwnd) };
            return Err(error);
        }

        Ok(Self { hwnd })
    }

    pub(crate) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub(crate) fn resize_client(&self, size: GuiSize) -> io::Result<()> {
        let (width, height) = outer_size(size)?;
        // SAFETY: hwnd is owned and remains valid until Drop.
        let succeeded = unsafe {
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                0,
                0,
                width,
                height,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
        if succeeded == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub(crate) fn show(&self, visible: bool) {
        // SAFETY: hwnd is owned and live. Restore a minimized shell when Ghost explicitly opens it.
        unsafe {
            let command = if visible {
                if IsIconic(self.hwnd) != 0 {
                    SW_RESTORE
                } else {
                    SW_SHOW
                }
            } else {
                SW_HIDE
            };
            ShowWindow(self.hwnd, command);
        }
    }
}

impl Drop for DetachedChildWindow {
    fn drop(&mut self) {
        // SAFETY: destruction occurs on the same CLAP main thread that created the window. The
        // NativeClapMain lifecycle destroys the embedded child GUI before this parent is dropped.
        unsafe {
            if IsWindow(self.hwnd) != 0 {
                DestroyWindow(self.hwnd);
            }
        }
    }
}

unsafe extern "system" fn detached_child_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_CLOSE {
        // A child CLAP GUI is parented into this HWND. Destroying the shell from the title-bar X
        // would invalidate that parent behind the plugin's back, so X behaves like Ghost's Hide UI
        // action. The shell remains available for a later Show UI and is destroyed only after the
        // child receives gui.destroy().
        ShowWindow(hwnd, SW_HIDE);
        return 0;
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}

fn outer_size(client_size: GuiSize) -> io::Result<(i32, i32)> {
    let mut rect = windows_sys::Win32::Foundation::RECT {
        left: 0,
        top: 0,
        right: i32::try_from(client_size.width)
            .map_err(|_| io::Error::other("child width is too large"))?,
        bottom: i32::try_from(client_size.height)
            .map_err(|_| io::Error::other("child height is too large"))?,
    };
    // SAFETY: rect is valid and the style matches CreateWindowExW above.
    let succeeded = unsafe { AdjustWindowRectEx(&mut rect, CHILD_WINDOW_STYLE, 0, 0) };
    if succeeded == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((rect.right - rect.left, rect.bottom - rect.top))
}
