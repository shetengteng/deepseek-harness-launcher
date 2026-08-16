//! 崩溃恢复策略（设计 §5.5 / PR-017）。
//!
//! 规则：
//! 1. dsh 意外退出 → `crash_counter += 1`，记 `last_crash_at`
//! 2. 距上次崩溃 ≥ `CRASH_WINDOW` → 视为新的一轮崩溃，counter 归 1
//! 3. `crash_counter < CRASH_RETRY_LIMIT` → 自动重启 current
//! 4. `>= CRASH_RETRY_LIMIT` → 上报前端弹窗（回滚 known_good / 继续重试 / 退出）
//! 5. 用户主动启动 Host → counter 清零

use chrono::{DateTime, Utc};

use crate::state::AppState;

/// 崩溃自动重试上限。达到后停止自动重启，弹窗交给用户决策。
pub const CRASH_RETRY_LIMIT: u32 = 3;

/// 崩溃窗口：两次崩溃间隔小于该值视为同一轮崩溃。
pub const CRASH_WINDOW_SECS: i64 = 5 * 60;

/// 记录一次崩溃后的决策。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashDecision {
    /// 继续自动重启 current 版本。
    RestartCurrent,
    /// 达到重试上限，交给用户决策。
    LimitReached,
}

/// 记录一次崩溃并给出决策。不落盘，调用方负责 `state.save()`。
pub fn record_crash(state: &mut AppState, now: DateTime<Utc>) -> CrashDecision {
    // 距上次崩溃超过窗口 → 新一轮崩溃，counter 归 1；否则累加
    let within_window = state
        .last_crash_at
        .map_or(true, |last| (now - last).num_seconds() < CRASH_WINDOW_SECS);
    state.crash_counter = if within_window {
        state.crash_counter.saturating_add(1)
    } else {
        1
    };
    state.last_crash_at = Some(now);

    if state.crash_counter < CRASH_RETRY_LIMIT {
        CrashDecision::RestartCurrent
    } else {
        CrashDecision::LimitReached
    }
}

/// 用户主动启动 Host：崩溃计数清零。
pub fn reset_crash_counter(state: &mut AppState) {
    state.crash_counter = 0;
    state.last_crash_at = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_crash_restarts() {
        let mut state = AppState::new();
        let d = record_crash(&mut state, Utc::now());
        assert_eq!(d, CrashDecision::RestartCurrent);
        assert_eq!(state.crash_counter, 1);
    }

    #[test]
    fn crashes_below_limit_keep_restarting() {
        let mut state = AppState::new();
        let t0 = Utc::now();
        record_crash(&mut state, t0);
        let d = record_crash(&mut state, t0 + chrono::Duration::seconds(30));
        assert_eq!(d, CrashDecision::RestartCurrent);
        assert_eq!(state.crash_counter, 2);
    }

    #[test]
    fn third_crash_reaches_limit() {
        let mut state = AppState::new();
        let t0 = Utc::now();
        record_crash(&mut state, t0);
        record_crash(&mut state, t0 + chrono::Duration::seconds(30));
        let d = record_crash(&mut state, t0 + chrono::Duration::seconds(60));
        assert_eq!(d, CrashDecision::LimitReached);
        assert_eq!(state.crash_counter, CRASH_RETRY_LIMIT);
    }

    #[test]
    fn crash_after_window_starts_new_episode() {
        let mut state = AppState::new();
        let t0 = Utc::now();
        record_crash(&mut state, t0);
        record_crash(&mut state, t0 + chrono::Duration::seconds(30));
        // 10 分钟后的崩溃是新一轮，counter 归 1
        let d = record_crash(&mut state, t0 + chrono::Duration::seconds(600));
        assert_eq!(d, CrashDecision::RestartCurrent);
        assert_eq!(state.crash_counter, 1);
    }

    #[test]
    fn reset_clears_counter_and_timestamp() {
        let mut state = AppState::new();
        record_crash(&mut state, Utc::now());
        reset_crash_counter(&mut state);
        assert_eq!(state.crash_counter, 0);
        assert!(state.last_crash_at.is_none());
    }
}
