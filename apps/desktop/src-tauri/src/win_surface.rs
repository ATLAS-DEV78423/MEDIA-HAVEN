#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub fn validate_rect(x: f64, y: f64, width: f64, height: f64) -> Option<SurfaceRect> {
    let values = [x, y, width, height];
    if !values.iter().all(|v| v.is_finite()) {
        return None;
    }
    if x < 0.0 || y < 0.0 || width < 1.0 || height < 1.0 {
        return None;
    }
    if values.iter().any(|v| *v > 100_000.0) {
        return None;
    }
    Some(SurfaceRect {
        x: x as i32,
        y: y as i32,
        width: width as i32,
        height: height as i32,
    })
}

pub fn to_physical(rect: SurfaceRect, dpi: u32) -> SurfaceRect {
    let scale = dpi as f64 / 96.0;
    let each = |v: i32| -> i32 {
        ((v as f64 * scale).round() as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32
    };
    SurfaceRect {
        x: each(rect.x),
        y: each(rect.y),
        width: each(rect.width).max(1),
        height: each(rect.height).max(1),
    }
}

use windows::Win32::Foundation::{HINSTANCE, HWND};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, SetWindowPos, ShowWindow, HWND_TOP, SWP_NOACTIVATE,
    SWP_NOZORDER, SW_HIDE, SW_SHOW, WINDOW_EX_STYLE, WINDOW_STYLE, WS_CHILD, WS_EX_NOACTIVATE,
    WS_EX_TRANSPARENT, WS_VISIBLE,
};

// windows 0.62 implements BitOr for these newtypes non-const; combine raw bits.
pub const CHILD_EX_STYLE: WINDOW_EX_STYLE =
    WINDOW_EX_STYLE(WS_EX_TRANSPARENT.0 | WS_EX_NOACTIVATE.0);
pub const CHILD_STYLE: WINDOW_STYLE = WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0);

#[derive(Debug, thiserror::Error)]
pub enum SurfaceError {
    #[error("could not create the video surface")]
    Create,
}

pub struct Surface {
    hwnd: HWND,
}

impl Surface {
    pub fn create(parent: usize, rect: SurfaceRect, dpi: u32) -> Result<Self, SurfaceError> {
        let physical = to_physical(rect, dpi);
        unsafe {
            let module = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .map_err(|_| SurfaceError::Create)?;
            let class = windows::core::w!("STATIC");
            let hwnd = CreateWindowExW(
                CHILD_EX_STYLE,
                class,
                windows::core::w!("mediahaven-video-surface"),
                CHILD_STYLE,
                physical.x,
                physical.y,
                physical.width,
                physical.height,
                Some(HWND(parent as *mut core::ffi::c_void)),
                None,
                Some(HINSTANCE::from(module)),
                None,
            )
            .map_err(|_| SurfaceError::Create)?;
            Ok(Self { hwnd })
        }
    }

    pub fn hwnd(&self) -> usize {
        self.hwnd.0 as usize
    }

    pub fn position(&self, rect: SurfaceRect, dpi: u32) {
        let p = to_physical(rect, dpi);
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOP),
                p.x,
                p.y,
                p.width,
                p.height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    pub fn show(&self, show: bool) {
        unsafe {
            let _ = ShowWindow(self.hwnd, if show { SW_SHOW } else { SW_HIDE });
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

unsafe impl Send for Surface {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_non_finite_negative_and_absurd_rects() {
        assert!(validate_rect(f64::NAN, 0.0, 10.0, 10.0).is_none());
        assert!(validate_rect(0.0, -1.0, 10.0, 10.0).is_none());
        assert!(validate_rect(0.0, 0.0, 0.0, 10.0).is_none());
        assert!(validate_rect(0.0, 0.0, 10.0, 200_000.0).is_none());
    }
    #[test]
    fn converts_logical_rect_to_physical_pixels() {
        let physical = to_physical(validate_rect(10.0, 20.0, 100.0, 50.0).unwrap(), 144);
        assert_eq!(
            physical,
            SurfaceRect {
                x: 15,
                y: 30,
                width: 150,
                height: 75
            }
        );
    }

    #[test]
    fn child_window_styles_are_transparent_to_hit_testing() {
        // The child must not steal focus/clicks from the WebView2 control bands.
        assert_eq!(CHILD_EX_STYLE & WS_EX_TRANSPARENT, WS_EX_TRANSPARENT);
        assert_ne!(CHILD_STYLE & WS_CHILD, WINDOW_STYLE::default());
    }
}
