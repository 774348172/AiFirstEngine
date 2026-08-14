use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, SetActiveWindow, SetFocus, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, PostMessageW, SetCursorPos,
    SetForegroundWindow, ShowWindow, SW_RESTORE, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsAuthorityClickReceipt {
    pub client_x: i32,
    pub client_y: i32,
    pub screen_x: i32,
    pub screen_y: i32,
    pub target_pid: u32,
    pub foreground_verified: bool,
    pub sent_input_count: u32,
}

pub fn send_authority_primary_click(
    window: &Window,
    client_x: i32,
    client_y: i32,
) -> Result<WindowsAuthorityClickReceipt, String> {
    let handle = window
        .window_handle()
        .map_err(|error| format!("authority_input.window_handle_failed:{error}"))?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("authority_input.non_win32_window".to_string());
    };
    let hwnd = handle.hwnd.get() as HWND;
    let mut target_pid = 0;
    let _thread_id = unsafe { GetWindowThreadProcessId(hwnd, &mut target_pid) };
    if target_pid != std::process::id() {
        return Err(format!(
            "authority_input.pid_mismatch:expected={}:actual={target_pid}",
            std::process::id()
        ));
    }
    let foreground_verified = activate_authority_window(hwnd);
    if !foreground_verified {
        return Err(foreground_mismatch_diagnostic(hwnd, target_pid));
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
    let mut point = POINT {
        x: client_x,
        y: client_y,
    };
    if unsafe { ClientToScreen(hwnd, &mut point) } == 0 {
        return Err("authority_input.client_to_screen_failed".to_string());
    }
    if unsafe { SetCursorPos(point.x, point.y) } == 0 {
        return Err("authority_input.set_cursor_failed".to_string());
    }
    let inputs = [
        mouse_input(MOUSEEVENTF_LEFTDOWN),
        mouse_input(MOUSEEVENTF_LEFTUP),
    ];
    let sent_input_count = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    if sent_input_count != inputs.len() as u32 {
        return Err(format!(
            "authority_input.send_input_incomplete:sent={sent_input_count}:expected={}",
            inputs.len()
        ));
    }
    Ok(WindowsAuthorityClickReceipt {
        client_x,
        client_y,
        screen_x: point.x,
        screen_y: point.y,
        target_pid,
        foreground_verified,
        sent_input_count,
    })
}

pub fn begin_authority_primary_drag(
    window: &Window,
    start_client_x: i32,
    start_client_y: i32,
) -> Result<WindowsAuthorityClickReceipt, String> {
    let receipt = prepare_authority_mouse_target(window, start_client_x, start_client_y)?;
    send_mouse_inputs(&[mouse_input(MOUSEEVENTF_LEFTDOWN)])?;
    Ok(WindowsAuthorityClickReceipt {
        sent_input_count: 1,
        ..receipt
    })
}

pub fn begin_authority_primary_click(
    window: &Window,
    client_x: i32,
    client_y: i32,
) -> Result<WindowsAuthorityClickReceipt, String> {
    let receipt = prepare_authority_mouse_target(window, client_x, client_y)?;
    post_authority_primary_message(window, client_x, client_y, WM_MOUSEMOVE, 0)?;
    post_authority_primary_message(window, client_x, client_y, WM_LBUTTONDOWN, 1)?;
    Ok(WindowsAuthorityClickReceipt {
        sent_input_count: 1,
        ..receipt
    })
}

pub fn finish_authority_primary_click(
    window: &Window,
    client_x: i32,
    client_y: i32,
) -> Result<(), String> {
    post_authority_primary_message(window, client_x, client_y, WM_LBUTTONUP, 0)
}

pub fn move_authority_primary_drag(
    window: &Window,
    end_client_x: i32,
    end_client_y: i32,
) -> Result<(), String> {
    let hwnd = window_hwnd(window)?;
    let mut end = POINT {
        x: end_client_x,
        y: end_client_y,
    };
    if unsafe { ClientToScreen(hwnd, &mut end) } == 0 {
        return Err("authority_input.drag_client_to_screen_failed".to_string());
    }
    if unsafe { SetCursorPos(end.x, end.y) } == 0 {
        return Err("authority_input.drag_set_cursor_failed".to_string());
    }
    Ok(())
}

pub fn finish_authority_primary_drag() -> Result<(), String> {
    send_mouse_inputs(&[mouse_input(MOUSEEVENTF_LEFTUP)])?;
    Ok(())
}

pub fn send_authority_mouse_wheel(
    window: &Window,
    client_x: i32,
    client_y: i32,
    delta: i32,
) -> Result<WindowsAuthorityClickReceipt, String> {
    send_authority_mouse_input(window, client_x, client_y, &[mouse_wheel_input(delta)])
}

fn send_authority_mouse_input(
    window: &Window,
    client_x: i32,
    client_y: i32,
    inputs: &[INPUT],
) -> Result<WindowsAuthorityClickReceipt, String> {
    let receipt = prepare_authority_mouse_target(window, client_x, client_y)?;
    let sent_input_count = send_mouse_inputs(inputs)?;
    Ok(WindowsAuthorityClickReceipt {
        sent_input_count,
        ..receipt
    })
}

fn prepare_authority_mouse_target(
    window: &Window,
    client_x: i32,
    client_y: i32,
) -> Result<WindowsAuthorityClickReceipt, String> {
    let hwnd = window_hwnd(window)?;
    let mut target_pid = 0;
    let _thread_id = unsafe { GetWindowThreadProcessId(hwnd, &mut target_pid) };
    if target_pid != std::process::id() {
        return Err(format!(
            "authority_input.pid_mismatch:expected={}:actual={target_pid}",
            std::process::id()
        ));
    }
    let foreground_verified = activate_authority_window(hwnd);
    if !foreground_verified {
        return Err(foreground_mismatch_diagnostic(hwnd, target_pid));
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
    let mut point = POINT {
        x: client_x,
        y: client_y,
    };
    if unsafe { ClientToScreen(hwnd, &mut point) } == 0 {
        return Err("authority_input.client_to_screen_failed".to_string());
    }
    if unsafe { SetCursorPos(point.x, point.y) } == 0 {
        return Err("authority_input.set_cursor_failed".to_string());
    }
    Ok(WindowsAuthorityClickReceipt {
        client_x,
        client_y,
        screen_x: point.x,
        screen_y: point.y,
        target_pid,
        foreground_verified,
        sent_input_count: 0,
    })
}

fn window_hwnd(window: &Window) -> Result<HWND, String> {
    let handle = window
        .window_handle()
        .map_err(|error| format!("authority_input.window_handle_failed:{error}"))?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("authority_input.non_win32_window".to_string());
    };
    Ok(handle.hwnd.get() as HWND)
}

fn activate_authority_window(hwnd: HWND) -> bool {
    for _ in 0..20 {
        unsafe {
            ShowWindow(hwnd, SW_RESTORE);
            BringWindowToTop(hwnd);
            SetActiveWindow(hwnd);
            SetFocus(hwnd);
            SetForegroundWindow(hwnd);
        }
        if unsafe { GetForegroundWindow() == hwnd } {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    false
}

fn foreground_mismatch_diagnostic(target_hwnd: HWND, target_pid: u32) -> String {
    let foreground_hwnd = unsafe { GetForegroundWindow() };
    let mut foreground_pid = 0;
    if !foreground_hwnd.is_null() {
        unsafe {
            GetWindowThreadProcessId(foreground_hwnd, &mut foreground_pid);
        }
    }
    format!(
        "authority_input.target_not_foreground:target_hwnd={target_hwnd:?}:target_pid={target_pid}:foreground_hwnd={foreground_hwnd:?}:foreground_pid={foreground_pid}"
    )
}

fn post_authority_primary_message(
    window: &Window,
    client_x: i32,
    client_y: i32,
    message: u32,
    key_state: usize,
) -> Result<(), String> {
    let hwnd = window_hwnd(window)?;
    let lparam = ((client_x as u16 as u32) | ((client_y as u16 as u32) << 16)) as isize;
    if unsafe { PostMessageW(hwnd, message, key_state, lparam) } == 0 {
        return Err(format!(
            "authority_input.post_message_failed:message={message}"
        ));
    }
    Ok(())
}

fn send_mouse_inputs(inputs: &[INPUT]) -> Result<u32, String> {
    let sent_input_count = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    if sent_input_count != inputs.len() as u32 {
        return Err(format!(
            "authority_input.send_input_incomplete:sent={sent_input_count}:expected={}",
            inputs.len()
        ));
    }
    Ok(sent_input_count)
}

fn mouse_input(flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn mouse_wheel_input(delta: i32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: delta as u32,
                dwFlags: MOUSEEVENTF_WHEEL,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
