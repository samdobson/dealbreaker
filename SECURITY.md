# Security Policy

## Supported Versions

Security fixes are provided for the latest `1.x` minor release:

| Version | Supported |
| --- | --- |
| `1.3.x` | Yes |
| `< 1.3` | No |

## Reporting a Vulnerability

Please **do not** open a public GitHub issue for security vulnerabilities.

Instead, report vulnerabilities privately through [GitHub Security Advisories](https://github.com/samdobson/dealbreaker/security/advisories/new).

Include as much detail as possible:

- Affected crate or binding (`dealbreaker`, `dealbreaker-cli`, `dealbreaker-ffi`, `dealbreaker-wasm`, `dealbreaker-py`)
- Minimal reproduction input (e.g., JSON rules/state, C FFI call sequence, or CLI invocation)
- Expected vs. actual behavior (e.g., crash, memory safety violation, unbounded memory/CPU on malformed input)

## Scope

Security issues of particular interest include:

- Memory safety or undefined behavior in the C ABI (`crates/dealbreaker-ffi`) or Python bindings (`crates/dealbreaker-py`)
- Panics, stack overflows, or uncontrolled resource exhaustion when parsing untrusted rules, states, or steps JSON

You can expect an initial acknowledgement within 7 days.
