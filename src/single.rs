//! 单实例（P3c）：命名互斥体判重 + 命名事件唤回。
//!
//! 为什么不让第二个实例自己把已有窗口抢到前台：Windows 默认**禁止后台进程抢前台**
//! （顶多闪两下任务栏）。抢前台的正确姿势是「已经在前台的那个进程自己做，或者由它把许可让出去」。
//! 所以第二个实例只做两件事：`AllowSetForegroundWindow(ASFW_ANY)` 把许可让出去，
//! 再 `SetEvent` 敲一下唤回事件，然后自己安静退出 —— 新窗口不开。
//!
//! 已有实例里有一个**阻塞等待**的线程（不是轮询）：事件一到就置原子标记 +
//! `request_repaint()`，下一帧 app 读到标记 → 把面板（哪怕藏在托盘里）唤回前台。
//!
//! 名字用 `Local\`（当前登录会话）而不是 `Global\`：免管理员，也不会撞上别的会话。

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Arc;

use eframe::egui;

/// 互斥体名：拿到 = 本进程是第一个实例
pub const MUTEX_NAME: &str = "Local\\Lexideck.SingleInstance";
/// 唤回事件名：第二个实例敲它，已有实例收到就把窗口拿到前台
pub const WAKE_NAME: &str = "Local\\Lexideck.Wake";

/// 进程级的句柄（拿住不放：互斥体一关就等于「退出了」；事件句柄等待线程一直在用）。
/// 0 = 还没有。
static MUTEX: AtomicIsize = AtomicIsize::new(0);
static EVENT: AtomicIsize = AtomicIsize::new(0);
/// 有人敲过门（等待线程置上，app 每帧 `take_wake` 取走）
static WAKE: AtomicBool = AtomicBool::new(false);

/// 等待线程的存活开关；`Watcher` 一 drop 就停车。
pub struct Watcher {
    alive: Arc<AtomicBool>,
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Relaxed);
    }
}

/// app 每帧问一次：有人敲过门吗（有则取出并清掉）。
/// 纯原子操作，任何平台都能调。
pub fn take_wake() -> bool {
    WAKE.swap(false, Ordering::Relaxed)
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::System::Threading::{
        CreateEventW, CreateMutexW, OpenEventW, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// 启动时判定：true = 本进程是第一个实例；false = 已经有实例在跑
    /// （调用方应当改去 `wake_existing()` 然后退出）。
    /// 句柄故意「拿了不放」：互斥体活到进程结束，由系统回收。
    pub fn install() -> bool {
        unsafe {
            let m = CreateMutexW(std::ptr::null(), 0, wide(MUTEX_NAME).as_ptr());
            if m.is_null() {
                // 建不出来（理论上不会）：当单实例保护不存在，照常启动，别把用户挡在外面
                return true;
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                CloseHandle(m);
                return false;
            }
            MUTEX.store(m as isize, Ordering::Relaxed);

            // 唤回事件：自动重置（等一次就复位，不用手动清）。建不出来只是少了「唤到前台」，
            // 不影响启动。
            let e = CreateEventW(std::ptr::null(), 0, 0, wide(WAKE_NAME).as_ptr());
            if !e.is_null() {
                EVENT.store(e as isize, Ordering::Relaxed);
            }
            true
        }
    }

    /// 第二个实例：把「允许抢前台」的许可让出去，再敲一下唤回事件。
    /// 让许可这一步失败也不影响：已有实例照样会被叫醒、把面板放回屏幕里，只是可能不置顶。
    pub fn wake_existing() {
        unsafe {
            AllowSetForegroundWindow(ASFW_ANY);
            let e = OpenEventW(EVENT_MODIFY_STATE, 0, wide(WAKE_NAME).as_ptr());
            if !e.is_null() {
                SetEvent(e);
                CloseHandle(e);
            }
        }
    }

    /// 已有实例：起一个等待线程（阻塞等事件；只有退出检查用小超时）。
    pub fn watch(ctx: &egui::Context) -> Watcher {
        let alive = Arc::new(AtomicBool::new(true));
        let h = EVENT.load(Ordering::Relaxed);
        if h == 0 {
            return Watcher { alive };
        }
        let (alive2, ctx2) = (alive.clone(), ctx.clone());
        std::thread::spawn(move || unsafe {
            let h = h as HANDLE;
            while alive2.load(Ordering::Relaxed) {
                // 事件一来就醒；500ms 超时只是回头看一眼 alive（顺带兜住句柄失效的情况）
                if WaitForSingleObject(h, 500) == WAIT_OBJECT_0 {
                    WAKE.store(true, Ordering::Relaxed);
                    ctx2.request_repaint();
                }
            }
        });
        Watcher { alive }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    /// 非 Windows：没有单实例判定（照常启动），唤回是空操作
    pub fn install() -> bool {
        true
    }
    pub fn wake_existing() {}
    pub fn watch(_ctx: &egui::Context) -> Watcher {
        Watcher {
            alive: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub use imp::{install, wake_existing, watch};

#[cfg(test)]
mod tests {
    use super::*;

    /// 同名互斥体拿第二次：Windows 会回 ERROR_ALREADY_EXISTS → install() 必须报「已有实例」。
    /// （第一次拿到的那份故意不释放，所以这个断言在任何执行顺序下都成立。）
    #[cfg(windows)]
    #[test]
    fn second_install_sees_running_instance() {
        let _first = install();
        assert!(!install(), "第二次拿同名互斥体应当是「已有实例在跑」");
    }

    /// 门铃的整条链路：install 建事件 → watch 起等待线程 → wake_existing 敲 → take_wake 取到
    #[cfg(windows)]
    #[test]
    fn wake_round_trip() {
        let _ = install();
        let ctx = egui::Context::default();
        let _w = watch(&ctx);
        assert!(!take_wake(), "没人敲门时应当是 false");
        wake_existing();
        let mut ok = false;
        for _ in 0..40 {
            if take_wake() {
                ok = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(ok, "敲门后应当取到标记");
        assert!(!take_wake(), "取过一次就清掉了");
    }
}
