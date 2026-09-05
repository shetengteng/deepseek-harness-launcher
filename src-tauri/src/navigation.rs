use std::sync::RwLock;

use tauri::{plugin::TauriPlugin, Manager, Runtime};
use url::Url;

const EXTERNAL_LINK_BRIDGE: &str = r#"
(() => {
  const loopbackHosts = new Set(["127.0.0.1", "localhost"]);
  if (
    window.top === window ||
    window.parent !== window.top ||
    window.location.protocol !== "http:" ||
    !loopbackHosts.has(window.location.hostname)
  ) {
    return;
  }

  window.addEventListener("click", (event) => {
    if (event.defaultPrevented || event.button !== 0 || event.altKey) return;

    const anchor = event
      .composedPath()
      .find((node) => node instanceof HTMLAnchorElement);
    if (!(anchor instanceof HTMLAnchorElement) || !anchor.href) return;

    const target = new URL(anchor.href, window.location.href);
    if (
      (target.protocol !== "http:" && target.protocol !== "https:") ||
      target.origin === window.location.origin
    ) {
      return;
    }

    event.preventDefault();
    event.stopImmediatePropagation();
    window.top.postMessage(
      { type: "dsh:open-external", href: target.href },
      "*",
    );
  }, true);
})();
"#;

/// 仅允许壳页（内置协议 origin 或启动时注册的前端回环 origin）与当前 Host
/// 就绪行声明的精确 dsh origin 导航。
#[derive(Default)]
pub(crate) struct NavigationPolicy {
    dsh_origin: RwLock<Option<Url>>,
    launcher_origin: RwLock<Option<Url>>,
}

impl NavigationPolicy {
    pub(crate) fn activate_launcher_origin(&self, origin: &str) {
        let parsed = Url::parse(origin).expect("launcher origin is validated before activation");
        *self
            .launcher_origin
            .write()
            .expect("navigation policy lock poisoned") = Some(parsed);
    }

    pub(crate) fn activate_dsh_origin(&self, origin: &str) {
        let parsed = Url::parse(origin).expect("host origin was validated before activation");
        *self
            .dsh_origin
            .write()
            .expect("navigation policy lock poisoned") = Some(parsed);
    }

    pub(crate) fn clear_dsh_origin(&self) {
        *self
            .dsh_origin
            .write()
            .expect("navigation policy lock poisoned") = None;
    }

    pub(crate) fn current_dsh_origin(&self) -> Option<String> {
        self.dsh_origin
            .read()
            .expect("navigation policy lock poisoned")
            .as_ref()
            .map(ToString::to_string)
    }

    fn allows(&self, url: &Url) -> bool {
        is_builtin_launcher_url(url)
            || self
                .launcher_origin
                .read()
                .expect("navigation policy lock poisoned")
                .as_ref()
                .is_some_and(|origin| same_origin(origin, url))
            || self
                .dsh_origin
                .read()
                .expect("navigation policy lock poisoned")
                .as_ref()
                .is_some_and(|origin| same_origin(origin, url))
    }
}

pub(crate) fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("navigation-policy")
        .js_init_script_on_all_frames(EXTERNAL_LINK_BRIDGE)
        .on_navigation(|webview, url| {
            webview
                .app_handle()
                .state::<crate::commands::SharedState>()
                .navigation
                .allows(url)
        })
        .build()
}

fn is_builtin_launcher_url(url: &Url) -> bool {
    (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
        || (matches!(url.scheme(), "http" | "https") && url.host_str() == Some("tauri.localhost"))
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(value: &str) -> Url {
        Url::parse(value).unwrap()
    }

    #[test]
    fn allows_only_launcher_origins_before_dsh_starts() {
        let policy = NavigationPolicy::default();

        assert!(policy.allows(&url("tauri://localhost/")));
        assert!(policy.allows(&url("https://tauri.localhost/")));
        assert!(!policy.allows(&url("http://127.0.0.1:48123/")));
        assert!(!policy.allows(&url("https://example.com/")));
    }

    #[test]
    fn allows_only_the_registered_frontend_origin() {
        let policy = NavigationPolicy::default();
        policy.activate_launcher_origin("http://127.0.0.1:1420");

        assert!(policy.allows(&url("http://127.0.0.1:1420/")));
        assert!(policy.allows(&url("http://127.0.0.1:1420/assets/app.js")));
        assert!(!policy.allows(&url("http://127.0.0.1:1421/")));
        assert!(!policy.allows(&url("http://localhost:1420/")));
        assert!(!policy.allows(&url("https://127.0.0.1:1420/")));
    }

    #[test]
    fn allows_exact_dsh_origin_and_rejects_other_loopback_ports() {
        let policy = NavigationPolicy::default();
        policy.activate_dsh_origin("http://127.0.0.1:48123");

        assert!(policy.allows(&url("http://127.0.0.1:48123/projects/demo")));
        assert!(policy.allows(&url("http://127.0.0.1:48123/?tab=recent")));
        assert!(!policy.allows(&url("http://127.0.0.1:48124/")));
        assert!(!policy.allows(&url("http://localhost:48123/")));
        assert!(!policy.allows(&url("https://127.0.0.1:48123/")));
    }

    #[test]
    fn clearing_dsh_origin_revokes_previous_host() {
        let policy = NavigationPolicy::default();
        policy.activate_dsh_origin("http://localhost:48123");
        policy.clear_dsh_origin();

        assert!(!policy.allows(&url("http://localhost:48123/")));
        assert_eq!(policy.current_dsh_origin(), None);
    }

    #[test]
    fn exposes_the_active_dsh_origin_for_browser_opening() {
        let policy = NavigationPolicy::default();
        policy.activate_dsh_origin("http://127.0.0.1:48123/");

        assert_eq!(
            policy.current_dsh_origin().as_deref(),
            Some("http://127.0.0.1:48123/")
        );
    }
}
