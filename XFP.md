# XFP maintained Agent Browser

Base: upstream v0.38.1 (aff6125c023b810ea3f2e5deec5379e9a4270bdc).
Patch ported from https://github.com/vercel-labs/agent-browser/pull/1695
(PR head be85b8de34d1c26dfc8960598f9b0fe84a3653f3).

All internal and explicit tab creation requests set background=true. Switching
an automation target and auto-connect no longer implicitly bring Chrome forward.
The fork also exposes the existing native bringtofront action through the CLI
(the upstream v0.38.1 parser did not expose it). Use it for human handoff. This does not promise
that first browser launch or operating-system dialogs cannot activate a window.

Maintenance branch: xfp. Keep upstream changes separate and port reviewed fixes.
Version the root package, cli/Cargo.toml and cli/Cargo.lock together using
0.38.1-xfp.N. Push an xfp-v0.38.1-N tag to build macOS arm64/x64 and Windows x64.
The XFP native release workflow embeds the dashboard, builds native binaries,
and publishes a self-contained npm tarball. There is no postinstall download or
fallback to upstream binaries. The desktop app pins the release tarball URL and
its pnpm integrity hash. This package targets the XFP desktop platforms only.

Once upstream releases this behavior, verify background and explicit foreground
operations before moving the desktop app back to an upstream package.
