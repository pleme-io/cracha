# cracha — repo-level agent instructions

> Brazilian-Portuguese for "badge." This repo implements **crachá**,
> the typed authorization primitive of saguão. ASCII-folded for repo /
> crate / chart names.

## Frame

This repo is governed by the saguão architecture. Read first:

- [`pleme-io/theory/SAGUAO.md`](https://github.com/pleme-io/theory/blob/main/SAGUAO.md) §III.2 — what crachá *is*
- [`blackmatter-pleme/skills/saguao/SKILL.md`](https://github.com/pleme-io/blackmatter-pleme/blob/main/skills/saguao/SKILL.md) — how to operate it
- The Compounding Directive in `pleme-io/CLAUDE.md` — solve once, load-bearing fixes only, models stay current

## What this repo owns

- The typed `AccessPolicy` IR (Rust types with `#[derive(TataraDomain)]`)
- The `(defcrachá …)` Lisp authoring form (registered via TataraDomain)
- The kube-rs controller that reconciles `AccessPolicy` CRDs into an in-memory authz index
- The gRPC `Authorize(user, service, verb) → Decision` API consumed by vigia
- The REST `GET /accessible-services?user=<sub>` API consumed by varanda
- The Helm chart `lareira-cracha` deploying the controller + API to the control-plane cluster

## What this repo does NOT own

- **Identity** — that's `pleme-io/passaporte`. crachá's authz decision assumes the user identity is already verified by passaporte's JWT.
- **Enforcement** — that's `pleme-io/vigia`. crachá serves decisions; vigia applies them at the cluster ingress.
- **Per-resource ACL** — saguão's authz is service-level, not row-level. Per-resource ACL lives inside the application itself (e.g., Immich's own ACL).

## Conventions

- **Workspace member crates** are named `cracha-<concern>` (cracha-core, cracha-controller, cracha-api). Adding a fourth crate (e.g., cracha-cli) follows the same pattern.
- **TataraDomain derive** on every public IR type. The `(defcrachá …)` form must round-trip through serde without lossy normalization.
- **kube-rs** (not raw kube-derive) for CRD reconciliation; it's the canonical Rust K8s client in pleme-io.
- **tonic** for gRPC; **axum** for REST. Both share the same in-memory index via an `Arc<RwLock<AuthzIndex>>`.
- **No raw SQL / database.** crachá is stateless — the source of truth is the `AccessPolicy` CRD; the controller's in-memory index rebuilds on restart from the live cluster state. Persistence is a non-goal; if the controller restarts, it re-reads the CRDs.

## Build

```bash
nix develop
cargo build
cargo test
nix build .#cracha-controller
nix build .#cracha-api
```

## Pillar 1 reminder

This is a Rust + tatara-lisp project. **Never shell out** beyond the
3-line glue allowance. CLI subcommand wiring uses clap derive; config
loading uses shikumi; service lifecycle uses tsunagu. Every recurring
shape lifts to a substrate macro.

## Naming

`crachá` (with acute) in prose. `cracha` (ASCII) for everything that
appears in code, file paths, repo URLs, Helm chart names, K8s resource
names, gRPC service names. Same convention as `aplicacao` /
`aplicação`.
