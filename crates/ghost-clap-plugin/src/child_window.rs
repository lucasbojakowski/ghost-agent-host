//! Host-owned top-level Win32 container for children that cannot create floating editors.

use std::io;

use clack_extensions::gui::GuiSize;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DestroyWindow, IsWindow, SetWindowPos, ShowWindow,
    CW_USEDEFAULT, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SW_HIDE, SW_SHOW, WINDOW_EX_STYLE,
    WS_CLIPCHILDREN, WS_OVERLAPPEDWINDOW,
};

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
                WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
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
            Err(io::Error::last_os_error())
        } else {
            Ok(Self { hwnd })
        }
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
        // SAFETY: hwnd is owned and live.
        unsafe { ShowWindow(self.hwnd, if visible { SW_SHOW } else { SW_HIDE }) };
    }
}

impl Drop for DetachedChildWindow {
    fn drop(&mut self) {
        // SAFETY: destruction occurs on the same CLAP main thread that created the window.
        unsafe {
            if IsWindow(self.hwnd) != 0 {
                DestroyWindow(self.hwnd);
            }
        }
    }
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
    // SAFETY: rect is valid and the style values match CreateWindowExW above.
    let succeeded =
        unsafe { AdjustWindowRectEx(&mut rect, WS_OVERLAPPEDWINDOW, 0, 0 as WINDOW_EX_STYLE) };
    if succeeded == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((rect.right - rect.left, rect.bottom - rect.top))
}
