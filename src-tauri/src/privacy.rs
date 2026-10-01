use crate::foreground::foreground_window;

const SENSITIVE_PROCESS_NAMES: &[&str] = &[
    "keepass.exe",
    "keepassxc.exe",
    "1password.exe",
    "bitwarden.exe",
    "lastpass.exe",
    "dashlane.exe",
    "nordpass.exe",
    "roboform.exe",
    "veracrypt.exe",
];

const SENSITIVE_TITLE_KEYWORDS: &[&str] = &[
    "password",
    "banking",
    "online banking",
    "bank of",
    "paypal",
    "routing number",
    "one-time passcode",
    "2fa",
    "wallet",
    "seed phrase",
];

pub struct PrivacyCheck {
    pub blocked: bool,
    pub reason: Option<String>,
}

/// Checked before every capture so sensitive on-screen content (password
/// managers, banking sites, etc.) is never silently sent to the VLM
/// provider. Denylist-based, matched against the foreground window's
/// process name and title.
pub fn check_foreground_window() -> PrivacyCheck {
    let Some(window) = foreground_window() else {
        return PrivacyCheck {
            blocked: false,
            reason: None,
        };
    };

    if SENSITIVE_PROCESS_NAMES.contains(&window.process_name.as_str()) {
        return PrivacyCheck {
            blocked: true,
            reason: Some(format!(
                "the active app ({}) is a password/security manager",
                window.process_name
            )),
        };
    }

    let title_lower = window.title.to_lowercase();
    if let Some(keyword) = SENSITIVE_TITLE_KEYWORDS
        .iter()
        .find(|kw| title_lower.contains(**kw))
    {
        return PrivacyCheck {
            blocked: true,
            reason: Some(format!(
                "the active window title (\"{}\") looks sensitive (matched \"{}\")",
                window.title, keyword
            )),
        };
    }

    PrivacyCheck {
        blocked: false,
        reason: None,
    }
}
