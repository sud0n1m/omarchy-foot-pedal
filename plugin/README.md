# Native Omarchy plugin

`Service.qml` owns one bundled Rust worker while the plugin is enabled.
`Panel.qml` is the theme-aware bar/popup UI and communicates over the local Unix
socket. Closing the panel does not stop pedal controls; disabling/removing the
plugin does. There are no install hooks or persistent systemd units.

The Rust source and primary documentation live at the repository root.
