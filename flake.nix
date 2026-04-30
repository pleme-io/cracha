{
  description = "crachá — typed authorization for the saguão fleet";

  nixConfig = {
    allow-import-from-derivation = true;
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    crate2nix.url = "github:nix-community/crate2nix";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    substrate = {
      url = "github:pleme-io/substrate";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
    };
    forge = {
      url = "github:pleme-io/forge";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
      inputs.substrate.follows = "substrate";
      inputs.crate2nix.follows = "crate2nix";
    };
    devenv = {
      url = "github:cachix/devenv";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs@{
    self,
    nixpkgs,
    crate2nix,
    flake-utils,
    substrate,
    forge,
    devenv,
    ...
  }:
    # Service-flake builder — single binary `cracha-api`, packaged as
    # an OCI image and pushed to ghcr.io/pleme-io/cracha. The
    # cracha-api process spawns cracha-controller's reconciler
    # in-process via tokio::spawn, so one image covers both
    # surfaces (REST/gRPC API + CRD reconciler). The
    # cracha-cli binary still ships as a GitHub-release artifact via
    # the workspace's separate cargo build, but the cluster runtime
    # only needs cracha-api.
    (import "${substrate}/lib/build/rust/service-flake.nix" {
      inherit nixpkgs substrate forge crate2nix;
    }) {
      inherit self;
      serviceName = "cracha";
      registry = "ghcr.io/pleme-io/cracha";
      packageName = "cracha-api";
      moduleDir = null;
      nixosModuleFile = null;
      # Image needs nothing extra — cracha-api links statically against
      # all its deps (sea-orm, axum, jsonwebtoken). A future enrichment
      # (e.g. tini for PID 1, ca-certificates for outbound HTTPS to
      # passaporte's JWKS) can be added here.
      extraContents = pkgs: with pkgs; [ cacert ];
      # cracha-proto's build.rs invokes tonic-build, which shells out
      # to protoc. The default crate2nix override adds protobuf to
      # tonic-build's OWN build inputs but not to consumer crates;
      # cracha-proto needs it on its OWN nativeBuildInputs so the
      # build script finds protoc on PATH at compile time.
      crateOverrides = {
        cracha-proto = oldAttrs: {
          nativeBuildInputs = (oldAttrs.nativeBuildInputs or [])
            ++ (with (import nixpkgs { system = "x86_64-linux"; }); [ protobuf ]);
        };
      };
    };
}
