# crachá — typed authorization for the saguão fleet

> Brazilian-Portuguese for "badge." The credential that says which
> rooms you can enter.

`crachá` is the **typed authorization primitive** of the saguão
fleet identity + authz + portal architecture. It owns "who can see
what" across every cluster in every location of the pleme-io
homelab fleet — as a typed Rust value, version-controlled in git,
authored as `(defcrachá …)`.

**Canonical architecture:** [`pleme-io/theory/SAGUAO.md`](https://github.com/pleme-io/theory/blob/main/SAGUAO.md) §III.2.

**Status:** scaffold. **Phase 5** of the saguão migration. Not yet
deployed. The interface and CRD shape are stable; the implementation
is in progress.

## What it is

Three crates in one workspace:

| Crate | Role |
|---|---|
| `cracha-core` | Typed `AccessPolicy` IR with `#[derive(TataraDomain)]`. Pure types + serde + validation. No I/O. |
| `cracha-controller` | kube-rs reconciler watching `AccessPolicy` CRDs across the fleet. Builds an in-memory authz index. |
| `cracha-api` | Axum-based gRPC + REST server. gRPC for vigia (`Authorize`); REST for varanda (`/accessible-services?user=…`). |

One Helm chart:

| Chart | What it deploys |
|---|---|
| `charts/lareira-cracha` | Controller + API as a single Deployment, ServiceMonitor, PrometheusRule, Cloudflare Tunnel ingress to `cracha.quero.cloud`, gated by passaporte (Authentik forward-auth via `pleme-lib.compliance.authn.oidc`). |

## Usage (target shape)

```clojure
(defcrachá
  :name family
  :members [drzln cousin wife mom dad]
  :grants
  [(grant :user drzln  :locations [* ] :clusters [* ] :services [*]                   :verbs [* ])
   (grant :user wife   :locations [bristol parnamirim] :clusters [rio mar]
          :services [photos jellyfin notes paperless] :verbs [read write])
   (grant :user cousin :locations [bristol] :clusters [rio]
          :services [chat photos jellyfin]            :verbs [read write])])
```

Renders to a typed `AccessPolicy` CRD, applied via Flux to the
control-plane cluster (today: rio). The controller picks it up
and serves the in-memory index to consumers.

## Consumers

- **vigia** (per-cluster forward-auth) — calls
  `Authorize(user, service, verb)` over gRPC for every gated
  request.
- **varanda** (family-facing PWA) — calls
  `GET /accessible-services?user=<sub>` over REST to render the
  user's portal manifest.

## Repo layout

```
cracha/
├── README.md                       (this file)
├── CLAUDE.md                       (per-repo agent instructions)
├── flake.nix                       (substrate rust-workspace-release)
├── Cargo.toml                      (workspace root)
├── Cargo.lock                      (TBD — run `cargo generate-lockfile`)
├── Cargo.nix                       (TBD — run crate2nix)
├── .envrc / .gitignore
├── crates/
│   ├── cracha-core/                (typed AccessPolicy IR)
│   ├── cracha-controller/          (kube-rs reconciler)
│   └── cracha-api/                 (gRPC + REST API)
├── charts/
│   └── lareira-cracha/             (Helm chart)
└── examples/
    └── family.lisp                 (canonical example access policy)
```

## Bootstrap

```bash
nix develop
cargo generate-lockfile             # create Cargo.lock
nix run github:nix-community/crate2nix -- generate   # create Cargo.nix
cargo build
```

## Cross-references

- [`SAGUAO.md` §III.2](https://github.com/pleme-io/theory/blob/main/SAGUAO.md)
- `blackmatter-pleme/skills/saguao/SKILL.md`
- Companion repos: `pleme-io/passaporte` (identity), `pleme-io/vigia` (data plane), `pleme-io/varanda` (PWA)

## License

MIT.
