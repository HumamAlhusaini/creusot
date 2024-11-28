use clap::*;
use serde::{Deserialize, Serialize};
use std::{error::Error, ffi::OsString, path::PathBuf};

#[derive(Debug, Parser, Serialize, Deserialize)]
pub struct CommonOptions {
    /// Determines how to format the spans in generated code to loading in Why3.
    ///
    /// [Relative] is better if the generated code is meant to be checked into VCS.
    /// [Absolute] means the files can easily be moved around your system and still work.
    /// [None] provides the clearest diffs.
    /// NB: spans pointing to the Rust standard library are thrown away in [Relative] mode,
    /// while they are kept in [Absolute] mode.
    #[clap(long, value_enum, default_value_t=SpanMode::Absolute, verbatim_doc_comment)]
    pub span_mode: SpanMode,
    #[clap(long)]
    /// Directory with respect to which (relative) spans should be relative to.
    /// Necessary when using --stdout with relative spans, not needed otherwise.
    pub spans_relative_to: Option<PathBuf>,
    #[clap(long)]
    /// Location that Creusot metadata for this crate should be emitted to.
    pub metadata_path: Option<String>,
    /// Tell creusot to disable metadata exports.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub export_metadata: bool,
    /// Print to stdout.
    #[clap(group = "output", long)]
    pub stdout: bool,
    /// Print to a file.
    #[clap(group = "output", long, env)]
    pub output_file: Option<PathBuf>,
    #[clap(group = "output", long, env)]
    pub output_dir: Option<PathBuf>,
    /// Disable output.
    #[clap(group = "output", long, default_value_t = false, action = clap::ArgAction::SetTrue)]
    pub check: bool,
    /// Output the generated code in a single file in output_dir.
    #[clap(long, default_value_t = false, action = clap::ArgAction::SetTrue)]
    pub monolithic: bool,
    /// Specify locations of metadata for external crates. The format is the same as rustc's `--extern` flag.
    #[clap(long = "creusot-extern", value_parser= parse_key_val::<String, String>, required=false)]
    pub extern_paths: Vec<(String, String)>,
    /// Use `result` as the trigger of definition and specification axioms of logic/ghost/predicate functions
    #[clap(long, default_value_t = false, action = clap::ArgAction::Set)]
    pub simple_triggers: bool,
}

#[derive(Debug, Parser, Serialize, Deserialize)]
#[clap(override_usage = "creusot-rustc [RUSTC_OPTIONS] -- [OPTIONS]")]
pub struct CreusotArgs {
    #[clap(flatten)]
    pub options: CommonOptions,
    /// Path to the Why3 binary
    #[arg(long, default_value_os_t = PathBuf::from("why3"))]
    pub why3_path: PathBuf,
    /// Specify an alternative location for Why3's configuration
    #[arg(long)]
    pub why3_config_file: PathBuf,
    #[command(subcommand)]
    pub subcommand: Option<CreusotSubCommand>,
}

#[derive(Debug, Subcommand, Serialize, Deserialize, Clone)]
pub enum CreusotSubCommand {
    Why3 {
        /// Why3 subcommand to run
        #[clap(value_enum)]
        command: Why3SubCommand,
        /// Extra arguments to pass to why3
        #[clap(default_value_t = String::default())]
        args: String,
    },
    /// Generates the documentation, including specs, logical functions, etc.
    Doc,
}

#[derive(Debug, Parser)]
pub struct CargoCreusotArgs {
    #[clap(flatten)]
    pub options: CommonOptions,
    /// Subcommand: why3, setup
    #[command(subcommand)]
    pub subcommand: Option<CargoCreusotSubCommand>,
    /// Additional flags to pass to the underlying cargo invocation.
    #[clap(last = true)]
    #[clap(global = true)]
    pub cargo_flags: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum CargoCreusotSubCommand {
    /// Setup and manage Creusot's installation
    #[command(arg_required_else_help(true))]
    Setup {
        #[command(subcommand)]
        command: SetupSubCommand,
    },
    #[command(flatten)]
    Creusot(CreusotSubCommand),
    Config(ConfigArgs),
    Prove(ProveArgs),
}

#[derive(Debug, ValueEnum, Serialize, Deserialize, Clone)]
pub enum Why3SubCommand {
    Prove,
    Ide,
    Replay,
}

#[derive(Debug, ValueEnum, Serialize, Deserialize, Clone, PartialEq)]
pub enum SetupManagedTool {
    AltErgo,
    Z3,
    CVC4,
    CVC5,
}

#[derive(Debug, ValueEnum, Serialize, Deserialize, Clone, PartialEq)]
pub enum SetupTool {
    Why3,
    Why3find,
    AltErgo,
    Z3,
    CVC4,
    CVC5,
}

fn default_provers_parallelism() -> usize {
    match std::thread::available_parallelism() {
        Ok(n) => n.get(),
        Err(_) => 1,
    }
}

#[derive(Debug, Parser, Clone)]
pub enum SetupSubCommand {
    /// Show the current status of the Creusot installation
    Status,
    /// Setup Creusot or update an existing installation
    Install {
        /// Maximum number of provers to run in parallel
        #[arg(long, default_value_t = default_provers_parallelism())]
        provers_parallelism: usize,
        /// Look-up <TOOL> from PATH instead of using the built-in version
        #[arg(long, value_name = "TOOL")]
        external: Vec<SetupManagedTool>,
        /// Do not error if <TOOL>'s version does not match the one expected by creusot
        #[arg(long, value_name = "TOOL")]
        no_check_version: Vec<SetupTool>,
    },
}

/// Parse a single key-value pair
fn parse_key_val<T, U>(s: &str) -> Result<(T, U), Box<dyn Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: Error + Send + Sync + 'static,
{
    let pos = s.find('=').ok_or_else(|| format!("invalid KEY=value: no `=` found in `{}`", s))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}

impl CreusotArgs {
    pub fn parse_from<I: Into<OsString> + Clone>(it: impl IntoIterator<Item = I>) -> Self {
        Parser::parse_from(it)
    }
}

impl CargoCreusotArgs {
    pub fn parse_from<I: Into<OsString> + Clone>(it: impl IntoIterator<Item = I>) -> Self {
        Parser::parse_from(it)
    }
}

#[derive(Debug, clap::ValueEnum, Clone, Deserialize, Serialize)]
pub enum SpanMode {
    Relative,
    Absolute,
    Off,
}

#[derive(Debug, Parser)]
pub struct ProveArgs {
    /// Run Why3 IDE on next unproved goal.
    #[clap(short = 'i', default_value_t = false, action = clap::ArgAction::SetTrue)]
    pub ide: bool,
    /// Files to prove; default to everything in `verif/`.
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Parser)]
pub struct ConfigArgs {
    /// All arguments are forwarded to `why3find config`; see `why3find config --help` for a list of options.
    #[clap(allow_hyphen_values = true)]
    pub args: Vec<String>,
}
