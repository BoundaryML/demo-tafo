# Releases

`Package macOS app` runs manually or on a `v*` tag. It builds an Apple Silicon
`.app`, embeds the BAML telemetry credential in its generated native SDK, bundles
the matching runtime dylib, verifies the ad-hoc signature, smoke tests the bundled
runtime with Cloud disabled, and uploads a ZIP plus SHA-256 checksum. Tags also
publish a GitHub prerelease. Model credentials are never included.

Configure these repository settings before the first run:

- Secret `TAFO_TELEMETRY_KEY`: a Boundary API key with **Write (upload)** access
  to exactly the intended development environment, no query access.
- Variable `TAFO_TELEMETRY_PROJECT`: `org/project`.
- Variable `TAFO_TELEMETRY_ENVIRONMENT`: the exact environment name verified by BAML.

The pinned BAML CLI verifies these assertions before SDK generation; a missing,
overprivileged, revoked, or mismatched key fails the build. Saved personal login
sessions and model keys must never be used as the embedding credential. The
write-only key is intentionally recoverable from a shipped app; it is not a
secret from download recipients. Revoke it in Cloud and rebuild to rotate it.

Only the package step receives the embedding secret, and pull-request CI does not
receive it. Builds currently target development, not production. Changing the
configured destination is an explicit release decision. App traces can include
app identities and semantic request text; HTTP bodies are disabled. Tracking can
be paused from the native menu.

The ZIP installs by dragging Tafo.app into Applications. Semantic requests use the
user's own JEV key from the private Application Support `.env`. The app preserves
its existing Smart Switch bundle identifier and state directory. It requires
Accessibility permission for the shortcut. The release is ad-hoc signed and not
notarized; consistent Developer ID signing and notarization need Apple developer
credentials and are not configured here.

Local offline packaging: `./scripts/package.sh` (Cloud is off by default).
