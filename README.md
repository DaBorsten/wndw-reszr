# Wndw Reszr

A small Windows app that sets the position and size of any other window to exact values.

Pick a window from the list, type or drag X, Y, width and height, and the window follows. Values are in pixels or in percent of the monitor's work area, which is the screen without the taskbar.

## What it does

- Lists the visible windows of other programs with their icons and a search field.
- Sets position and size in pixels or percent. X and Y refer to the top-left corner or to the window center.
- Applies changes live while you drag a value, or only when you press Apply.
- Keeps the window inside the work area. At the monitor edge it either stops growing or shifts the window so it can keep growing.
- Has presets for the left, right, top and bottom half, the full work area, and centering.
- Follows the window when you move it yourself, and shows target and current position in a preview.
- Comes with a light and a dark theme and a German and an English interface.

## Requirements

- Windows 10 or 11
- A graphics driver with OpenGL 2.0 or newer

Windows of programs that run as administrator show up greyed out. Start Wndw Reszr as administrator to change them.

There is no Linux or macOS version. All window handling goes through the Win32 API.

## Build

You need the [Rust toolchain](https://rustup.rs) with the MSVC target and the Windows SDK, which ships with the Visual Studio Build Tools.

```
cargo build --release
```

The result is `target\release\windows-resizer.exe`. It is a single file and needs no installer.

## Settings

Theme and language are stored in `%APPDATA%\windows-resizer\settings.txt`. Delete the file to go back to the system defaults.

## License

[MIT](LICENSE)
