## Summary

- What does this PR change and why?

## Checklist

- [ ] `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass
- [ ] `cargo test --workspace --release` passes
- [ ] If adding or changing a rule mechanic: added a brute-force differential test in `crates/dealbreaker/tests/differential.rs`
- [ ] If changing public rules, options, or CLI flags: regenerated schemas/reference docs (`./scripts/gen-docs.sh`) and updated TypeScript types if needed
- [ ] Preserves 1.x SemVer compatibility (`cargo-semver-checks`)
