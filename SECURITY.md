# Security

## Reporting a vulnerability

Report it privately, not in a public issue or pull request:

    https://github.com/DrJesseGlass/ic-dev-kit-rs/security/advisories/new

That form opens a draft advisory only the maintainer and you can see. Say
what you found, how to reproduce it (a failing test is the best form), and
which version, features and target (host or `wasm32-unknown-unknown`) it
needs. Expect an acknowledgement within a week.

A bug that is not a security problem is an ordinary issue:
https://github.com/DrJesseGlass/ic-dev-kit-rs/issues/new

## What counts

ic-dev-kit-rs's promise is that the guards it provides and the endpoints
its macros generate admit exactly the callers their documentation says,
and that what an untrusted caller sends cannot take more from the canister
than the documented limits allow. Anything that breaks that promise is in
scope, in particular:

- `auth`: `is_authorized` passing for a caller outside the authorized set,
  an endpoint from `export_auth_endpoints!` that changes or reads the set
  without passing that guard, the last principal removed through
  `remove_principal`, or saved bytes that decode to a different set than
  the one `save_to_bytes` wrote;
- `telemetry` (`telemetry` feature): metrics or logs read by a caller who
  is not a controller, an `auth` admin or a monitoring principal
  (`is_monitoring_authorized`), or the monitoring list changed by one who
  is not a controller or an `auth` admin (`is_monitoring_admin`);
- the generated endpoints: any endpoint from `generate_upload_endpoints!`,
  `generate_model_endpoints!` or `export_telemetry_endpoints!` reachable
  without the guard the macro was given, and `setup_model` or
  `reset_generation` reachable by a caller `auth` does not authorize;
- `large_objects`: one owner reading, changing or clearing another owner's
  buffers, or bytes buffered past the per-owner or the total cap
  (`set_max_bytes_per_owner`, `set_max_total_bytes`);
- `http`: a request that `Router` dispatches to a handler whose route does
  not match its method and path, a path or header that `extract_path`,
  `matches_pattern`, `extract_params` or `get_header` reads differently
  than the gateway that delivered it, or a streaming token the types
  decode to something other than what was encoded;
- `storage` (`storage` feature): a value `load_candid` returns that was not
  the one saved under that key;
- a trap on untrusted input in any of the above, or in `text_generation`
  and `model_server` on a caller's prompt and parameters. A trapped update
  rolls back, but a trap an outsider can cause at will on an endpoint
  others depend on is a report.

## What does not

- The public arms of the macros. `generate_upload_endpoints!()` with no
  guard and `generate_model_endpoints!` without `generate_guard` are open
  to everyone, as the README says (Macros).
- The functions under the macros check nobody. `auth::add_principal`,
  `telemetry::add_monitoring_principal` and every `large_objects` function
  act on the principal they are handed; calling them from an unguarded
  endpoint, or passing an `owner` that is not the caller, is the caller's
  bug.
- `auth::init_from_saved` authorizes the caller of the upgrade when the
  saved bytes are missing or do not decode (README, Upgrade Persistence).
  That caller is a controller, who could install any code anyway. A way
  for a non-controller to make the saved bytes undecodable is a report.
- State on the Wasm heap is lost on upgrade unless the canister saves it:
  the authorized set, the telemetry state, in-flight upload buffers. That
  is documented in each module.
- Bytes buffered up to the caps, and any amount once a canister has set a
  cap to `None`. The defaults are 1 GiB per owner and 2 GiB in total; a
  canister that cannot afford that sets them lower.
- `http::cors_preflight_response` allows every origin, and
  `extract_bearer_token` returns the token without judging it. Both do
  what their names say; the policy is the canister's.
- `intercanister` calls whatever canister and method it is given, with the
  cycles it is given.
- What a model generates, and what a model's weights or tokenizer do when
  loaded. `setup_model` is an admin endpoint and loads what an admin put
  in the registry.
- The `telemetry` feature linking a second, private ic-cdk 0.19 (README,
  Installation). Two copies is the documented state, not a finding.
- A dependency advisory with no reachable path in this code. Dependabot
  files those (`.github/dependabot.yml`); if you believe a path exists,
  that is a report.
- A vulnerability in a canister that uses this crate, unless the crate is
  the cause. ic-git has its own policy:
  https://github.com/DrJesseGlass/ic-git/blob/main/SECURITY.md

## Supported versions

Only the latest release on crates.io. A fix is a new version; older ones
are not patched.

## How a fix ships

A fix is a release to crates.io (README, Releasing), with a CHANGELOG
entry and the affected versions yanked where the flaw warrants it. The
canisters that consume the crate take it by a version bump, which changes
their module hashes, so each ships it by its own release path; for ic-git
that is a reproducible build and a governor vote, which takes days, not
hours. The advisory is published, with credit to the reporter if wanted,
once the fixed version is on crates.io and the known consumers have had
the chance to take it.
