// cracha-cli — author-side renderer for the saguão typed forms.
//
// Subcommands:
//   cracha render fleet   <fleet.lisp>   --out dir/    — renders every
//                                                       per-cluster
//                                                       artifact + a
//                                                       fleet summary.
//   cracha render cluster <cluster.lisp> --out dir/    — one cluster's
//                                                       artifacts.
//   cracha render policy  <policy.lisp>  --out file    — one
//                                                       AccessPolicy
//                                                       CRD YAML.

mod render;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use cracha_core::{AccessPolicySpec, Cluster, Fleet};
use std::path::PathBuf;
use tatara_lisp::reader;

#[derive(Parser, Debug)]
#[command(name = "cracha")]
#[command(version)]
#[command(about = "Author-side renderer for saguão typed forms", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Render typed Lisp forms to deployable artifacts.
    Render {
        #[command(subcommand)]
        target: RenderTarget,
    },
    /// Print which Lisp keywords this CLI knows about.
    Keywords,
}

#[derive(Subcommand, Debug)]
enum RenderTarget {
    /// Render a (deffleet …) declaration to per-cluster artifacts.
    Fleet {
        input: PathBuf,
        #[arg(long, short, default_value = "out")]
        out: PathBuf,
    },
    /// Render a single (defcluster …) declaration to artifacts.
    Cluster {
        input: PathBuf,
        #[arg(long, short, default_value = "out")]
        out: PathBuf,
    },
    /// Render a (defcracha …) declaration to an AccessPolicy CRD YAML.
    Policy {
        input: PathBuf,
        #[arg(long, short, default_value = "out/access-policy.yaml")]
        out: PathBuf,
        #[arg(long, default_value = "cracha")]
        namespace: String,
    },
}

fn main() -> Result<()> {
    cracha_core::register_lisp_surfaces();
    let args = Args::parse();

    match args.command {
        Command::Keywords => {
            for kw in tatara_lisp::domain::registered_keywords() {
                println!("{kw}");
            }
            Ok(())
        }
        Command::Render { target } => match target {
            RenderTarget::Fleet { input, out } => render_fleet(&input, &out),
            RenderTarget::Cluster { input, out } => render_cluster(&input, &out),
            RenderTarget::Policy { input, out, namespace } => {
                render_policy(&input, &out, &namespace)
            }
        },
    }
}

/// Read a single Lisp form from a file and compile it to T via
/// TataraDomain. Surfaces tatara-lisp's parse + compile errors with
/// file context.
fn read_lisp<T>(path: &std::path::Path) -> Result<T>
where
    T: tatara_lisp::domain::TataraDomain,
{
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let forms = reader::read(&source)
        .with_context(|| format!("parsing {}", path.display()))?;
    if forms.is_empty() {
        anyhow::bail!("{}: no Lisp forms found", path.display());
    }
    if forms.len() > 1 {
        anyhow::bail!(
            "{}: expected exactly one top-level form, got {}",
            path.display(),
            forms.len()
        );
    }
    let value = T::compile_from_sexp(&forms[0])
        .with_context(|| format!("compiling {} to {}", path.display(), std::any::type_name::<T>()))?;
    Ok(value)
}

fn render_fleet(input: &std::path::Path, out: &std::path::Path) -> Result<()> {
    let fleet: Fleet = read_lisp(input)?;
    let written = render::fleet::render_all(&fleet, out)
        .with_context(|| format!("rendering fleet → {}", out.display()))?;
    println!("rendered {} files into {}:", written.len(), out.display());
    for f in written {
        println!("  - {f}");
    }
    Ok(())
}

fn render_cluster(input: &std::path::Path, out: &std::path::Path) -> Result<()> {
    let cluster: Cluster = read_lisp(input)?;
    let written = render::cluster::render_all(&cluster, out)
        .with_context(|| format!("rendering cluster → {}", out.display()))?;
    println!("rendered {} files into {}:", written.len(), out.display());
    for f in written {
        println!("  - {f}");
    }
    Ok(())
}

fn render_policy(
    input: &std::path::Path,
    out: &std::path::Path,
    namespace: &str,
) -> Result<()> {
    let policy: AccessPolicySpec = read_lisp(input)?;
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    render::policy::render_to_file(&policy, namespace, out)
        .with_context(|| format!("rendering policy → {}", out.display()))?;
    println!("rendered AccessPolicy → {}", out.display());
    Ok(())
}
