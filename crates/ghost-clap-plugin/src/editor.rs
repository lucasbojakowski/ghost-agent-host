use std::io;

use clack_extensions::gui::{GuiApiType, GuiConfiguration, GuiSize, PluginGuiImpl, Window};
use clack_plugin::prelude::{PluginError, PluginMainThread};
use egui::{FullOutput, ViewportOutput};
use egui_baseview::baseview::dpi::{PhysicalSize, Size};
use egui_baseview::baseview::{WindowHandle, WindowScalePolicy};
use egui_baseview::{EguiWindow, EguiWindowSettings, ExtraOutputCommands};
use ghost_ui::{GhostUi, DEFAULT_EDITOR_HEIGHT, DEFAULT_EDITOR_WIDTH};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, FindWindowExW, GetParent, IsWindow, SetWindowPos, ShowWindow, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOZORDER, SW_HIDE, SW_SHOW,
};

const EDITOR_TITLE: &str = "Ghost Agent Host";

pub struct GhostEditorMainThread {
    created: bool,
    window: Option<EditorWindow>,
    size: GuiSize,
    scale: f64,
}

impl Default for GhostEditorMainThread {
    fn default() -> Self {
        Self {
            created: false,
            window: None,
            size: GuiSize {
                width: DEFAULT_EDITOR_WIDTH,
                height: DEFAULT_EDITOR_HEIGHT,
            },
            scale: 1.0,
        }
    }
}

impl PluginMainThread<'_, ()> for GhostEditorMainThread {}

impl PluginGuiImpl for GhostEditorMainThread {
    fn is_api_supported(&mut self, configuration: GuiConfiguration) -> bool {
        configuration.api_type == GuiApiType::WIN32 && !configuration.is_floating
    }

    fn get_preferred_api(&mut self) -> Option<GuiConfiguration<'_>> {
        Some(GuiConfiguration {
            api_type: GuiApiType::WIN32,
            is_floating: false,
        })
    }

    fn create(&mut self, configuration: GuiConfiguration) -> Result<(), PluginError> {
        if !self.is_api_supported(configuration) {
            return Err(PluginError::Message(
                "Ghost Agent Host only supports embedded Win32 editors",
            ));
        }
        if self.created {
            return Err(PluginError::Message("Ghost editor already exists"));
        }

        self.created = true;
        Ok(())
    }

    fn destroy(&mut self) {
        self.window.take();
        self.created = false;
    }

    fn set_scale(&mut self, scale: f64) -> Result<(), PluginError> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err(PluginError::Message("Invalid editor scale"));
        }
        self.scale = scale;
        Ok(())
    }

    fn get_size(&mut self) -> Option<GuiSize> {
        Some(self.size)
    }

    fn set_size(&mut self, size: GuiSize) -> Result<(), PluginError> {
        if size.width == 0 || size.height == 0 {
            return Err(PluginError::Message("Editor size must be non-zero"));
        }

        self.size = size;
        if let Some(window) = &self.window {
            window.resize(size)?;
        }
        Ok(())
    }

    fn set_parent(&mut self, parent: Window) -> Result<(), PluginError> {
        if !self.created {
            return Err(PluginError::Message("Editor has not been created"));
        }
        if self.window.is_some() {
            return Err(PluginError::Message("Ghost editor already has a parent"));
        }

        let parent_hwnd = parent
            .as_win32_hwnd()
            .ok_or(PluginError::Message("Host did not provide a Win32 parent"))?
            as HWND;
        let settings = EguiWindowSettings::new()
            .with_tile(EDITOR_TITLE)
            .with_size(Size::Physical(PhysicalSize {
                width: self.size.width,
                height: self.size.height,
            }))
            .with_scale_policy(WindowScalePolicy::ScaleFactor(self.scale));
        // SAFETY: The host guarantees that its parent window remains valid through gui.destroy.
        let parent_handle = unsafe { parent.borrow_handle_unchecked() }
            .map_err(|error| io::Error::other(error.to_string()))?;
        let handle = EguiWindow::open_parented(
            &parent_handle,
            settings,
            GhostUi::default(),
            |_context, _commands: &mut ExtraOutputCommands, _state| {},
            |_output: &FullOutput, _viewport: &ViewportOutput, _state| {},
            |ui, _commands: &mut ExtraOutputCommands, state| state.show(ui),
        );

        let hwnd = find_editor_child(parent_hwnd).ok_or_else(|| {
            handle.close();
            io::Error::other("egui-baseview did not create the editor child window")
        })?;
        let editor = EditorWindow {
            hwnd,
            _handle: handle,
        };
        editor.show(false);
        self.window = Some(editor);
        Ok(())
    }

    fn set_transient(&mut self, _window: Window) -> Result<(), PluginError> {
        Err(PluginError::Message("Floating editors are not supported"))
    }

    fn show(&mut self) -> Result<(), PluginError> {
        self.window
            .as_ref()
            .ok_or(PluginError::Message("Editor does not have a parent"))?
            .show(true);
        Ok(())
    }

    fn hide(&mut self) -> Result<(), PluginError> {
        self.window
            .as_ref()
            .ok_or(PluginError::Message("Editor does not have a parent"))?
            .show(false);
        Ok(())
    }
}

impl Drop for GhostEditorMainThread {
    fn drop(&mut self) {
        self.destroy();
    }
}

struct EditorWindow {
    hwnd: HWND,
    _handle: WindowHandle,
}

impl EditorWindow {
    fn resize(&self, size: GuiSize) -> Result<(), PluginError> {
        let width = i32::try_from(size.width)
            .map_err(|_| PluginError::Message("Editor width is too large"))?;
        let height = i32::try_from(size.height)
            .map_err(|_| PluginError::Message("Editor height is too large"))?;
        // SAFETY: hwnd is the live child window owned by the baseview handle.
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
            return Err(io::Error::last_os_error().into());
        }
        Ok(())
    }

    fn show(&self, visible: bool) {
        // SAFETY: hwnd is the live child window owned by the baseview handle.
        unsafe {
            ShowWindow(self.hwnd, if visible { SW_SHOW } else { SW_HIDE });
        }
    }
}

impl Drop for EditorWindow {
    fn drop(&mut self) {
        // baseview's public close method posts a message. CLAP may unload the DLL immediately after
        // destroy returns, so destroy the same-thread child synchronously instead.
        // SAFETY: CLAP invokes GUI lifecycle methods on the same main thread that created hwnd.
        unsafe {
            if IsWindow(self.hwnd) != 0 {
                DestroyWindow(self.hwnd);
            }
        }
    }
}

fn find_editor_child(parent: HWND) -> Option<HWND> {
    let title: Vec<u16> = EDITOR_TITLE.encode_utf16().chain(Some(0)).collect();
    // SAFETY: parent is supplied by the host and title is a terminated UTF-16 string.
    let child = unsafe {
        FindWindowExW(
            parent,
            std::ptr::null_mut(),
            std::ptr::null(),
            title.as_ptr(),
        )
    };
    if child.is_null() || unsafe { GetParent(child) } != parent {
        None
    } else {
        Some(child)
    }
}
