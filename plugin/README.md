# Foot Pedal · Omarchy plugin

Native panel for the configurable Elgato Stream Deck Pedal service. Installed
in the right bar after Wave XLR; `foot-pedal` opens it from the application menu.

[Setup, restore and behavior](../docs/usage.md).
[Paper design](../docs/design/README.md).

`Panel.qml` uses the shared Omarchy `qs.Commons`/`qs.Ui` components and theme roles.
It connects directly to the user service through `Quickshell.Io.Socket`, with
no helper process. State updates arrive only when something changes. Closing the panel leaves
normal dispatch running, but releases its test session. Draft edits stay local
until Save & use. No generated Paper hex values are embedded in the plugin.

`sudonim.foot-pedal` IPC provides native open/close/toggle commands.
`sudonim.foot-pedal-status status` provides read-only diagnostics, including the
current draft, actual state, focused control and resolved theme. Command drafts
may contain private user-entered arguments; do not publish arbitrary diagnostics.
