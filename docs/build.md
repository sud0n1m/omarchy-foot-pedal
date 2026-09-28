# Build and verify the Rust worker

The native panel remains QML, which is Omarchy's plugin UI format. All background
input, action execution, configuration, local RPC, and the optional USB-rule
helper are Rust. No Python process is involved during normal use.

The checked-in `bin/foot-pedal` is a stripped Linux x86-64 executable built from
this source with Rust 1.98.1, Cargo.lock, and the release profile in Cargo.toml.
It dynamically links the standard glibc, libgcc_s and libudev libraries. Minimum
glibc is 2.39; this release targets Omarchy 4 on x86-64. A checksum is included
in `bin/SHA256SUMS`. It detects accidental changes, not independent authorship.

Install a Rust toolchain and libudev development files for your build machine
(Arch's systemd-libs supplies libudev). The pinned toolchain is declared in
`rust-toolchain.toml`.

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
PEDAL_BINARY=target/release/elgato-foot-pedal python3 -m unittest discover -s tests -v
sha256sum --check bin/SHA256SUMS
python3 -m unittest discover -s tests -v
omarchy plugin validate .
udevadm verify packaging/70-elgato-foot-pedal.rules
```

Python is used only by the black-box development tests. CI runs the Rust tests,
compiles the source, tests that build, and separately verifies/tests the bundled
binary on Ubuntu 24.04. CI uploads its own build for comparison. Builds on a
different distribution are not claimed to be byte-identical.

To replace the bundled binary after all checks pass:

```bash
cp target/release/elgato-foot-pedal bin/foot-pedal
sha256sum bin/foot-pedal > bin/SHA256SUMS
```

Do not copy over an executable while it is running; disable the installed plugin
before replacing its binary. Omarchy's Git update replaces files safely, and
normal enablement runs the bundled artifact without downloading or compiling.

[THIRD-PARTY-NOTICES.txt](../THIRD-PARTY-NOTICES.txt) contains dependency license
notices. The original pedal glyph in `icon.svg` and `preview.svg` is MIT licensed
with this project. `preview.png` is rendered from `preview.svg` using librsvg;
the marketplace currently uses this preview and initials beside the title,
rather than a custom title icon field.
