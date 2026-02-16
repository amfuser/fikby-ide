# Quick Start Guide - Theme Switching

## 🎨 How to Switch Themes in Fikby IDE

### Method 1: Keyboard Shortcut (Fastest)
Press `Ctrl+T` at any time to toggle between light and dark themes.

### Method 2: Menu
1. Click on **View** in the menu bar
2. Select **Toggle Theme**

## 📋 What Happens When You Toggle

When you switch themes, the following changes occur **instantly**:

✅ **Menubar** - Background and text colors change  
✅ **Line Numbers** - Gutter background and text colors update  
✅ **Text Editor** - Background and text colors switch  
✅ **Status Bar** - Bottom bar colors change  
✅ **Syntax Highlighting** - Code colors update to match theme  
✅ **All Open Tabs** - Every open file updates simultaneously  
✅ **Future Tabs** - New files will use the current theme  

## 🌞 Light Theme Colors

- **Background**: White/Light gray (`#ffffff`, `#f5f5f5`)
- **Text**: Dark gray/Black (`#333`, `#000`)
- **Gutter**: Light gray (`#efefef`)
- **Syntax**: base16-ocean.light color scheme

**Best for**: Bright environments, daytime coding, reduced eye strain in well-lit rooms

## 🌙 Dark Theme Colors

- **Background**: Dark gray/Black (`#1e1e1e`, `#2b2b2b`)
- **Text**: Light gray/White (`#d4d4d4`, `#e0e0e0`)
- **Gutter**: Dark gray (`#2b2b2b`)
- **Syntax**: base16-ocean.dark color scheme

**Best for**: Low-light environments, nighttime coding, reduced screen glare

## ⚡ Tips

- **Quick Switching**: Use `Ctrl+T` for instant theme changes
- **No Restart Required**: Changes apply immediately
- **Persistent**: Your theme choice affects all current and future tabs
- **Safe**: Theme switching is instant with no glitches or delays

## 🔧 Technical Details

For developers interested in the implementation:
- See `THEME_IMPLEMENTATION.md` for architecture details
- See `SECURITY_REVIEW.md` for security analysis
- See `src/tests/theme_mode.rs` for unit tests

## 💡 Default Theme

Fikby IDE starts in **Dark Theme** by default. Use `Ctrl+T` to switch to Light Theme if preferred.

---

**Need help?** Check the main [README.md](README.md) or [IMPLEMENTATION_COMPLETE.md](IMPLEMENTATION_COMPLETE.md) for more information.
