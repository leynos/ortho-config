//! Ensures configuration helpers are accessible via re-exports.

use ortho_config::{figment::Figment, uncased::UncasedStr};

#[test]
fn reexports_are_public() {
    let _figment = Figment::default();
    let _uncased = UncasedStr::new("key");
    #[cfg(any(unix, target_os = "redox"))]
    {
        use ortho_config::xdg::BaseDirectories;
        let _ = BaseDirectories::with_prefix("myapp");
    }
}
