use crate::config::ThemeMode;

/// Test that ThemeMode provides correct CSS for light theme
#[test]
fn theme_mode_light_css() {
    let mode = ThemeMode::Light;
    let css = mode.css();
    
    // Verify light theme CSS contains expected light colors
    assert!(css.contains("#f5f5f5"), "Light theme should contain light gray background");
    assert!(css.contains("#ffffff"), "Light theme should contain white background");
    assert!(css.contains("#efefef"), "Light theme should contain light gutter background");
}

/// Test that ThemeMode provides correct CSS for dark theme
#[test]
fn theme_mode_dark_css() {
    let mode = ThemeMode::Dark;
    let css = mode.css();
    
    // Verify dark theme CSS contains expected dark colors
    assert!(css.contains("#2b2b2b"), "Dark theme should contain dark gray background");
    assert!(css.contains("#1e1e1e"), "Dark theme should contain dark editor background");
    assert!(css.contains("#e0e0e0"), "Dark theme should contain light text color");
}

/// Test that ThemeMode provides correct syntax theme names
#[test]
fn theme_mode_syntax_theme_names() {
    assert_eq!(ThemeMode::Light.syntax_theme_name(), "base16-ocean.light");
    assert_eq!(ThemeMode::Dark.syntax_theme_name(), "base16-ocean.dark");
}

/// Test that ThemeMode enum has correct variants
#[test]
fn theme_mode_variants() {
    let light = ThemeMode::Light;
    let dark = ThemeMode::Dark;
    
    assert_ne!(light, dark, "Light and Dark should be different variants");
    
    // Test that we can match on variants
    match light {
        ThemeMode::Light => (),
        ThemeMode::Dark => panic!("Light variant should match Light"),
    }
    
    match dark {
        ThemeMode::Dark => (),
        ThemeMode::Light => panic!("Dark variant should match Dark"),
    }
}

/// Test that CSS contains all required UI element selectors
#[test]
fn theme_css_contains_all_selectors() {
    let light_css = ThemeMode::Light.css();
    let dark_css = ThemeMode::Dark.css();
    
    let required_selectors = vec![
        ".menubar",
        ".menubutton",
        ".right-button",
        ".gutter",
        ".editor-view",
        ".status",
    ];
    
    for selector in required_selectors {
        assert!(light_css.contains(selector), 
            "Light theme CSS should contain {} selector", selector);
        assert!(dark_css.contains(selector), 
            "Dark theme CSS should contain {} selector", selector);
    }
}
