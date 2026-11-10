//! The plugins' helpers at launch and on a consent change (add-public-release D5, D6;
//! add-plugin-architecture 3.3). One answer covers every helper: the first-run screen asks once.

use crate::settings::HookConsent;
use std::sync::Arc;
use terminescu_plugin_api::{Helper, PluginError};

/// A helper that failed, by id, with why.
pub type Failed = Vec<(&'static str, PluginError)>;

/// The launch step: an installed helper counts as consent, and with `On` every helper is
/// installed again, so its entry follows the CLI to where it now lives. Returns the answer.
pub fn at_launch(helpers: &[Arc<dyn Helper>], consent: HookConsent) -> (HookConsent, Failed) {
    let now = consent.at_launch(helpers.iter().any(|h| h.installed()));
    let failed = if now == HookConsent::On {
        apply(helpers, now)
    } else {
        Vec::new()
    };
    (now, failed)
}

/// Installs every helper for `On`, removes every one for `Off`; `Unasked` leaves them alone.
pub fn apply(helpers: &[Arc<dyn Helper>], consent: HookConsent) -> Failed {
    helpers
        .iter()
        .filter_map(|h| {
            let done = match consent {
                HookConsent::On => h.install(),
                HookConsent::Off => h.uninstall(),
                HookConsent::Unasked => return None,
            };
            done.err().map(|e| (h.id(), e))
        })
        .collect()
}

/// The failures as one message, every helper named; `None` when all went well.
pub fn describe(failed: &Failed) -> Option<String> {
    (!failed.is_empty()).then(|| {
        failed
            .iter()
            .map(|(id, e)| format!("{id}: {e}"))
            .collect::<Vec<_>>()
            .join("; ")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A helper that records what was asked of it.
    #[derive(Default)]
    struct Fake {
        on: Mutex<bool>,
        calls: Mutex<Vec<&'static str>>,
    }

    impl Helper for Fake {
        fn id(&self) -> &'static str {
            "fake"
        }
        fn label(&self) -> &'static str {
            "Fake"
        }
        fn description(&self) -> &'static str {
            ""
        }
        fn installed(&self) -> bool {
            *self.on.lock().unwrap()
        }
        fn install(&self) -> Result<(), PluginError> {
            self.calls.lock().unwrap().push("install");
            *self.on.lock().unwrap() = true;
            Ok(())
        }
        fn uninstall(&self) -> Result<(), PluginError> {
            self.calls.lock().unwrap().push("uninstall");
            *self.on.lock().unwrap() = false;
            Ok(())
        }
    }

    #[test]
    fn hook_off_leaves_settings() {
        let installed = Arc::new(Fake::default());
        *installed.on.lock().unwrap() = true;
        let list: Vec<Arc<dyn Helper>> = vec![installed.clone()];
        let (now, failed) = at_launch(&list, HookConsent::Off);
        assert_eq!(now, HookConsent::Off);
        assert!(failed.is_empty());
        assert!(installed.calls.lock().unwrap().is_empty(), "no edit");

        let fresh = Arc::new(Fake::default());
        let list: Vec<Arc<dyn Helper>> = vec![fresh.clone()];
        let (now, _) = at_launch(&list, HookConsent::Unasked);
        assert_eq!(now, HookConsent::Unasked);
        assert!(
            fresh.calls.lock().unwrap().is_empty(),
            "no consent, no edit"
        );

        let list: Vec<Arc<dyn Helper>> = vec![installed.clone()];
        let (now, _) = at_launch(&list, HookConsent::Unasked);
        assert_eq!(
            now,
            HookConsent::On,
            "an installed helper counts as consent"
        );
        assert_eq!(
            *installed.calls.lock().unwrap(),
            ["install"],
            "it moves to the current CLI"
        );

        assert!(apply(&list, HookConsent::Off).is_empty());
        assert!(!installed.installed());
    }

    #[test]
    fn every_failure_is_named() {
        assert_eq!(describe(&Vec::new()), None);
        let failed: Failed = vec![
            ("hook", PluginError::Failed("read-only".into())),
            ("git", PluginError::Timeout),
        ];
        assert_eq!(
            describe(&failed).as_deref(),
            Some("hook: read-only; git: the plugin took longer than its budget")
        );
    }
}
