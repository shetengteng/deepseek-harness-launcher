# Agent Note: dsh CLI shim

## Decision

The launcher installs an opt-in `dsh` shim in `~/.local/bin` instead of globally installing dsh through npm. The shim resolves the managed Node version and dsh `current` pointer at invocation time, so normal upgrades and rollbacks remain transparent.

## Profile compatibility

The shim preserves the caller's `DSH_HOME`. When unset, both the GUI-launched dsh process and the shim use dsh's default `~/.dsh`; existing Web profile plugins therefore remain shared.

## Validation

- Rust unit tests cover runtime validation, ownership protection, quoting, executable permissions, dynamic Node-version lookup, and argument forwarding.
- Vue component tests cover successful installation feedback and actionable errors.
