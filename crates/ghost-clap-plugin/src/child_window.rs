//! Host-owned top-level Win32 shell for embedded child CLAP editors.

use std::cell::RefCell;
use std::io;

use clack_extensions::gui::GuiSize;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow,
    GetActiveWindow, GetAncestor, GetForegroundWindow, IsChild, IsIconic, IsWindow, PostMessageW,
    SetWindowLongPtrW, SetWindowPos, SetWindowsHookExW, ShowWindow, UnhookWindowsHookEx, GA_ROOT,
    GWLP_WNDPROC, HHOOK, MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SW_HIDE,
    SW_RESTORE, SW_SHOW, WH_GETMESSAGE, WM_CLOSE, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
    WM_SYSKEYUP, WS_CAPTION, WS_CLIPCHILDREN, WS_EX_TOOLWINDOW, WS_MINIMIZEBOX, WS_OVERLAPPED,
    WS_SYSMENU, CW_USEDEFAULT,
};

const CHILD_WINDOW_STYLE: u32 =
    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;
const VK_SPACE_CODE: usize = 0x20;
const VK_F1_CODE: usize = 0x70;
const VK_F24_CODE: usize = 0x87;

#[derive(Clone, Copy)]
struct ShortcutRoute {
    shell: HWND,
    owner: HWND,
}

struct ShortcutBridgeState {
    hook: HHOOK,
    routes: Vec<ShortcutRoute>,
}

impl Default for ShortcutBridgeState {
    fn default() -> Self {
        Self {
            hook: std::ptr::null_mut(),
            routes: Vec::new(),
        }
    }
}

thread_local! {
    static SHORTCUT_BRIDGE: RefCell<ShortcutBridgeState> = RefCell::new(ShortcutBridgeState::default());
}

pub(crate) struct DetachedChildWindow {
    hwnd: HWND,
}

impl DetachedChildWindow {
    pub(crate) fn create(title: &str, client_size: GuiSize) -> io::Result<Self> {
        let class: Vec<u16> = "STATIC".encode_utf16().chain(Some(0)).collect();
        let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
        let (width, height) = outer_size(client_size)?;
        let owner = daw_owner_window();
        // SAFETY: class and title are terminated UTF-16 strings. Because this is a top-level
        // window, hWndParent is its owner rather than a child parent. Keeping the shell owned by
        // the DAW's active top-level window lets the host retain normal activation/accelerator
        // behavior while the vendor editor remains embedded outside Ghost's egui hierarchy.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                class.as_ptr(),
                title.as_ptr(),
                CHILD_WINDOW_STYLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                width,
                height,
                owner,
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
                detached_child_window_proc as *const () as usize as isize,
            )
        };
        if previous == 0 {
            let error = io::Error::last_os_error();
            unsafe { DestroyWindow(hwnd) };
            return Err(error);
        }

        if !owner.is_null() {
            if let Err(error) = register_shortcut_bridge(hwnd, owner) {
                unsafe { DestroyWindow(hwnd) };
                return Err(error);
            }
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
        unregister_shortcut_bridge(self.hwnd);
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

fn daw_owner_window() -> HWND {
    // The Show UI command originates from Ghost's editor on the DAW UI thread. At creation time the
    // active/foreground top-level window is therefore the wrapper/editor window that should own the
    // detached child. Prefer the thread-active window and fall back to the process foreground one.
    // SAFETY: these queries do not take ownership of the returned HWNDs.
    unsafe {
        let active = GetActiveWindow();
        let candidate = if active.is_null() {
            GetForegroundWindow()
        } else {
            active
        };
        if candidate.is_null() {
            return std::ptr::null_mut();
        }
        let root = GetAncestor(candidate, GA_ROOT);
        if root.is_null() { candidate } else { root }
    }
}

fn register_shortcut_bridge(shell: HWND, owner: HWND) -> io::Result<()> {
    SHORTCUT_BRIDGE.with(|bridge| {
        let mut bridge = bridge.borrow_mut();
        if bridge.hook.is_null() {
            // SAFETY: this is a thread-local WH_GETMESSAGE hook installed on the CLAP/DAW main
            // thread. The callback lives in this loaded plugin module for at least as long as the
            // registered detached windows, and Drop removes the hook before the plugin can unload.
            let hook = unsafe {
                SetWindowsHookExW(
                    WH_GETMESSAGE,
                    Some(shortcut_message_hook),
                    std::ptr::null_mut(),
                    GetCurrentThreadId(),
                )
            };
            if hook.is_null() {
                return Err(io::Error::last_os_error());
            }
            bridge.hook = hook;
        }
        bridge.routes.push(ShortcutRoute { shell, owner });
        Ok(())
    })
}

fn unregister_shortcut_bridge(shell: HWND) {
    SHORTCUT_BRIDGE.with(|bridge| {
        let mut bridge = bridge.borrow_mut();
        bridge.routes.retain(|route| route.shell != shell);
        if bridge.routes.is_empty() && !bridge.hook.is_null() {
            // SAFETY: hook was installed by this thread-local state on the same UI thread.
            unsafe {
                UnhookWindowsHookEx(bridge.hook);
            }
            bridge.hook = std::ptr::null_mut();
        }
    });
}

unsafe extern "system" fn shortcut_message_hook(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 && wparam == PM_REMOVE as usize && !lparam.is_negative() {
        let message = &*(lparam as *const MSG);
        if is_keyboard_message(message.message) && should_bridge_shortcut(message) {
            if let Some(owner) = shortcut_owner_for_target(message.hwnd) {
                // Re-post to the owning DAW window so its normal message loop/accelerator handling
                // sees transport and command shortcuts even while the vendor child owns focus. The
                // original message is left untouched so plugin text/key interaction still works.
                let _ = PostMessageW(owner, message.message, message.wParam, message.lParam);
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

fn shortcut_owner_for_target(target: HWND) -> Option<HWND> {
    if target.is_null() {
        return None;
    }
    SHORTCUT_BRIDGE.with(|bridge| {
        bridge.borrow().routes.iter().find_map(|route| {
            // SAFETY: route HWNDs are removed before their shell is destroyed; target is supplied
            // by the current thread's message queue.
            (target == route.shell || unsafe { IsChild(route.shell, target) } != 0)
                .then_some(route.owner)
        })
    })
}

fn is_keyboard_message(message: u32) -> bool {
    matches!(message, WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP)
}

fn should_bridge_shortcut(message: &MSG) -> bool {
    let key = message.wParam;
    if key == VK_SPACE_CODE || (VK_F1_CODE..=VK_F24_CODE).contains(&key) {
        return true;
    }
    if matches!(message.message, WM_SYSKEYDOWN | WM_SYSKEYUP) {
        return true;
    }
    if key == VK_CONTROL as usize || key == VK_MENU as usize || key == VK_SHIFT as usize {
        return false;
    }
    // Ctrl/Alt chords are overwhelmingly host commands in DAWs. Mirror them to the owner while
    // still dispatching the original to the child. Plain alphanumeric typing stays child-only.
    unsafe {
        key_is_down(VK_CONTROL as i32) || key_is_down(VK_MENU as i32)
    }
}

unsafe fn key_is_down(key: i32) -> bool {
    (GetKeyState(key) as u16 & 0x8000) != 0
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_bridge_keeps_plain_typing_child_local() {
        let mut message: MSG = unsafe { std::mem::zeroed() };
        message.message = WM_KEYDOWN;
        message.wParam = b'A' as usize;
        assert!(!should_bridge_shortcut(&message));

        message.wParam = VK_SPACE_CODE;
        assert!(should_bridge_shortcut(&message));

        message.wParam = VK_F1_CODE;
        assert!(should_bridge_shortcut(&message));
    }
}
