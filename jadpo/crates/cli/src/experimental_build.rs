//! Local entry point for the bounded shared-Rust fixture, not a general backend.
use jadpo_diagnostics::Diagnostic;
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, PartialEq)]
enum Target {
    Bun,
    Native,
    Wasm,
}

#[derive(Debug, PartialEq)]
struct Options {
    target: Target,
    project: Option<PathBuf>,
}

fn arguments_error() -> Diagnostic {
    Diagnostic::error("CLI_BUILD_ARGUMENTS").with_note(
        "expected: jadpo build [project] [--target bun|native|rust|wasm]; Bun requires a project",
    )
}

fn parse(arguments: &[String]) -> Result<Options, Diagnostic> {
    let mut target = None;
    let mut project = None;
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        let value = if argument == "--target" {
            Some(arguments.next().ok_or_else(arguments_error)?.as_str())
        } else {
            argument.strip_prefix("--target=")
        };
        if let Some(value) = value {
            if target.is_some() {
                return Err(arguments_error());
            }
            target = Some(match value {
                "bun" => Target::Bun,
                "native" | "rust" => Target::Native,
                "wasm" => Target::Wasm,
                _ => return Err(arguments_error()),
            });
        } else if argument.starts_with('-') || project.is_some() {
            return Err(arguments_error());
        } else {
            project = Some(PathBuf::from(argument));
        }
    }
    let target = target.unwrap_or(Target::Bun);
    if target == Target::Bun && project.is_none() {
        return Err(Diagnostic::error("CLI_PROJECT_REQUIRED"));
    }
    Ok(Options { target, project })
}

fn experiment_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|root| {
            root.join("jadpo/crates/cli/Cargo.toml").is_file()
                && root
                    .join("experiments/native-conformance/build.sh")
                    .is_file()
                && root
                    .join("experiments/wasm-exp1/fixture/app.jadpo")
                    .is_file()
        })
        .map(Path::to_path_buf)
}

fn require_fixture(root: &Path, project: &Path) -> Result<&'static str, Diagnostic> {
    for (fixture, experiment) in [
        ("experiments/wasm-exp1/fixture", "native-conformance"),
        ("experiments/auth-policy/fixture", "auth-policy"),
    ] {
        if let (Ok(expected), Ok(actual)) =
            (root.join(fixture).canonicalize(), project.canonicalize())
        {
            if expected == actual {
                return Ok(experiment);
            }
        }
    }
    Err(Diagnostic::error("CLI_BUILD_EXPERIMENT_SCOPE").with_note(
        "supported fixtures: experiments/wasm-exp1/fixture, experiments/auth-policy/fixture",
    ))
}

pub(super) fn run(arguments: &[String]) -> Result<(), Diagnostic> {
    let options = parse(arguments)?;
    if options.target == Target::Bun {
        return super::run_human_build(options.project.as_deref().expect("validated project"));
    }
    let cwd = env::current_dir().map_err(|error| {
        Diagnostic::error("CLI_BUILD_EXPERIMENT_UNAVAILABLE").with_note(error.to_string())
    })?;
    let root = experiment_root(&cwd)
        .or_else(|| {
            options.project.as_ref().and_then(|project| {
                project
                    .canonicalize()
                    .ok()
                    .and_then(|path| experiment_root(&path))
            })
        })
        .ok_or_else(|| Diagnostic::error("CLI_BUILD_EXPERIMENT_UNAVAILABLE"))?;
    let experiment = match &options.project {
        Some(project) => require_fixture(&root, project)?,
        None => "native-conformance",
    };
    let target = match options.target {
        Target::Native => "rust",
        Target::Wasm => "wasm",
        Target::Bun => unreachable!(),
    };
    eprintln!("Building experimental {experiment} fixture ({target}); outputs stay in the local experiment.");
    let mut command = Command::new("sh");
    command
        .arg(root.join(format!("experiments/{experiment}/build.sh")))
        .current_dir(&root);
    if experiment == "native-conformance" {
        command.arg("--target");
    }
    let status = command
        .arg(target)
        .status()
        .map_err(|error| Diagnostic::error("CLI_BUILD_TOOL_FAILED").with_note(error.to_string()))?;
    if !status.success() {
        return Err(Diagnostic::error("CLI_BUILD_TOOL_FAILED")
            .with_note(format!("build process: {status}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn default_build_stays_bun_and_requires_a_project() {
        assert_eq!(
            parse(&args(&["example"])).unwrap(),
            Options {
                target: Target::Bun,
                project: Some(PathBuf::from("example")),
            }
        );
        assert_eq!(parse(&[]).unwrap_err().code, "CLI_PROJECT_REQUIRED");
        assert!(parse(&args(&["--target=bun"])).is_err());
    }

    #[test]
    fn target_options_accept_both_positions_and_native_alias() {
        for name in ["native", "rust", "wasm", "bun"] {
            let before = parse(&args(&["--target", name, "example"])).unwrap();
            let after = parse(&args(&["example", &format!("--target={name}")])).unwrap();
            assert_eq!(before, after);
        }
        assert_eq!(
            parse(&args(&["--target", "native"])).unwrap().target,
            Target::Native
        );
        assert_eq!(
            parse(&args(&["--target=rust"])).unwrap().target,
            Target::Native
        );
        assert_eq!(
            parse(&args(&["--target=wasm"])).unwrap().target,
            Target::Wasm
        );
    }

    #[test]
    fn malformed_targets_never_fall_back_to_bun() {
        for values in [
            vec!["example", "--target"],
            vec!["example", "--target=unknown"],
            vec!["example", "--target=wasm", "--target=rust"],
            vec!["example", "--typo"],
            vec!["example", "another"],
        ] {
            assert_eq!(
                parse(&args(&values)).unwrap_err().code,
                "CLI_BUILD_ARGUMENTS"
            );
        }
    }

    #[test]
    fn experimental_builds_are_confined_to_the_checkout_fixture() {
        let root = experiment_root(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert_eq!(
            require_fixture(&root, &root.join("experiments/wasm-exp1/fixture")).unwrap(),
            "native-conformance"
        );
        assert_eq!(
            require_fixture(&root, &root.join("experiments/auth-policy/fixture")).unwrap(),
            "auth-policy"
        );
        for other in [
            "examples/jadpo-seed",
            "missing",
            "experiments/wasm-exp1/fixture/app.jadpo",
        ] {
            assert_eq!(
                require_fixture(&root, &root.join(other)).unwrap_err().code,
                "CLI_BUILD_EXPERIMENT_SCOPE"
            );
        }
    }
}
