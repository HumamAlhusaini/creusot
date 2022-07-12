use assert_cmd::prelude::*;
use std::{
    env,
    error::Error,
    fs::File,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::Command,
};

use similar::{ChangeTag, TextDiff};

use termcolor::*;

fn main() {
    let mut base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base_path.pop();

    let mut temp_file = base_path.clone();
    temp_file.push("target");
    temp_file.push("debug");
    temp_file.push("libcreusot_contracts.cmeta");

    let mut metadata_file = Command::cargo_bin("cargo-creusot").unwrap();
    metadata_file.current_dir(base_path);
    metadata_file
        .args(&["creusot", "--package", "creusot-contracts", "--features=contracts,num_bigint"])
        .env("CREUSOT_METADATA_PATH", &temp_file)
        .env("CREUSOT_OUTPUT_FILE", "/dev/null")
        .env("RUST_BACKTRACE", "1")
        .env("CREUSOT_CONTINUE", "true");

    if !metadata_file.status().expect("could not dump metadata for `creusot_contracts`").success() {
        // eprintln!("{}", String::from_utf8_lossy(&metadata_file.output().unwrap().stderr));
        std::process::exit(1);
    }

    should_fail("tests/should_fail/**/*.rs", |p| run_creusot(p, &temp_file.to_string_lossy()));
    should_succeed("tests/should_succeed/**/*.rs", |p| {
        run_creusot(p, &temp_file.to_string_lossy())
    });
}

fn run_creusot(file: &Path, contracts: &str) -> Option<std::process::Command> {
    let mut cmd = Command::cargo_bin("creusot-rustc").unwrap();
    cmd.current_dir(file.parent().unwrap());
    let mut base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base_path.pop();
    base_path.push("target");
    base_path.push("debug");

    let creusot_contract_path = base_path.join("libcreusot_contracts.rlib");
    let creusot_contract_path =
        creusot_contract_path.to_str().expect("invalid utf-8 in contract path");
    let creusot_contract_path = normalize_file_path(creusot_contract_path);

    let creusot_externs_json =
        serde_json::json!({ "creusot_contracts": normalize_file_path(contracts) });

    cmd.arg("-Zno-codegen");
    cmd.envs(env::vars());
    cmd.env("CREUSOT_EXPORT_METADATA", "false");
    cmd.env("CREUSOT_EXTERNS", creusot_externs_json.to_string());
    cmd.env("CREUSOT_STDOUT_OUTPUT", "1");
    cmd.env("CREUSOT_SPAN", "relative");
    cmd.args(&["--extern", &format!("creusot_contracts={}", creusot_contract_path)]);
    let header_line = BufReader::new(File::open(&file).unwrap()).lines().nth(0).unwrap().unwrap();

    if header_line.contains("UNBOUNDED") {
        cmd.env("CREUSOT_UNBOUNDED", "1");
    }

    if header_line.contains("UISKIP") {
        return None;
    }

    let mut dep_path = base_path;
    dep_path.push("deps");

    cmd.arg(format!("-Ldependency={}/", dep_path.display()));
    cmd.arg(file.file_name().unwrap());
    cmd.arg("--crate-type=lib");
    Some(cmd)
}

fn should_succeed<B>(s: &str, b: B)
where
    B: Fn(&Path) -> Option<std::process::Command>,
{
    glob_runner(s, b, should_succeed_case);
}

fn should_fail<B>(s: &str, b: B)
where
    B: Fn(&Path) -> Option<std::process::Command>,
{
    glob_runner(s, b, should_fail_case);
}

fn glob_runner<B, C>(s: &str, command_builder: B, differ: C)
where
    B: Fn(&Path) -> Option<std::process::Command>,
    C: Fn(std::process::Output, &Path, &Path) -> Result<(bool, Buffer), Box<dyn Error>>,
{
    let mut out = StandardStream::stdout(ColorChoice::Always);

    let mut test_count = 0;
    let mut test_failures = 0;
    let bless = std::env::args().any(|arg| arg == "--bless");
    let filter = std::env::args().nth(1);

    for entry in glob::glob(s).expect("Failed to read glob pattern") {
        test_count += 1;
        let entry = entry.unwrap();

        if let Some(ref filter) = filter {
            if !entry.to_str().map(|entry| entry.contains(filter)).unwrap_or(false) {
                continue;
            }
        }
        let output = match command_builder(&entry) {
            None => continue,
            Some(mut c) => c.output().unwrap(),
        };

        let stderr = entry.with_extension("stderr");
        let stdout = entry.with_extension("mlcfg");

        write!(&mut out, "Testing {} ... ", entry.display()).unwrap();

        if bless {
            out.set_color(ColorSpec::new().set_fg(Some(Color::Blue))).unwrap();
            writeln!(&mut out, "blessed").unwrap();
            out.reset().unwrap();

            if output.stdout.is_empty() {
                let _ = std::fs::remove_file(stdout);
            } else {
                std::fs::write(stdout, &output.stdout).unwrap();
            }
        } else {
            let (success, mut buf) = differ(output, &stdout, &stderr).unwrap();

            if success {
                out.set_color(ColorSpec::new().set_fg(Some(Color::Green))).unwrap();
                writeln!(&mut out, "ok").unwrap();
            } else {
                out.set_color(ColorSpec::new().set_fg(Some(Color::Red))).unwrap();
                writeln!(&mut out, "failure").unwrap();

                test_failures += 1;
            };
            out.reset().unwrap();

            buf.reset().unwrap();
            let wrt = BufferWriter::stdout(ColorChoice::Always);
            wrt.print(&buf).unwrap();
        }
    }

    if test_failures > 0 {
        out.set_color(ColorSpec::new().set_fg(Some(Color::Red))).unwrap();
        drop(out);
        panic!("{} failures out of {} tests", test_failures, test_count);
    }
}

fn compare_str(buf: &mut Buffer, got: &str, expect: &str) -> bool {
    use similar::Algorithm;
    use std::time::Duration;

    let got = normalize_newlines(got);
    let expect = normalize_newlines(expect);

    let result = TextDiff::configure()
        .newline_terminated(false)
        .timeout(Duration::from_millis(200))
        .algorithm(Algorithm::Patience)
        .diff_lines(&expect, &got);

    // let result = TextDiff::from_lines(expect, got);
    if result.ratio() == 1.0 {
        true
    } else {
        print_diff(buf, result);
        false
    }
}

/// Normalize new lines between linux/windows for consistency
///
/// Remove \r (for Windows)
fn normalize_newlines(input: impl Into<String>) -> String {
    let input: String = input.into();
    let input = input.replace("\r", "");
    input
}

/// Normalize file path between linux/windows for consistency
///
/// Replace \ by /  (for Windows)
fn normalize_file_path(input: impl Into<String>) -> String {
    let input: String = input.into();
    let input = input.replace("\\", "/");
    input
}

fn should_succeed_case(
    output: std::process::Output,
    stdout: &Path,
    _stderr: &Path,
) -> Result<(bool, Buffer), Box<dyn Error>> {
    let mut buf = Buffer::ansi();
    use std::str::from_utf8;
    match output.ok() {
        Ok(output) => {
            let expect = &std::fs::read(stdout).unwrap_or_else(|_| Vec::new());
            let gotten = &output.stdout;

            let success = compare_str(&mut buf, from_utf8(gotten)?, from_utf8(expect)?);
            if output.stderr.len() != 0 {
                write!(&mut buf, "{}", from_utf8(&output.stderr)?)?;
                Ok((false, buf))
            } else {
                Ok((success, buf))
            }
        }
        Err(err) => {
            let output = err.as_output().unwrap();
            let _ = compare_str(&mut buf, from_utf8(&output.stderr)?, "");
            Ok((false, buf))
        }
    }
}

fn should_fail_case(
    output: std::process::Output,
    _stdout: &Path,
    _stderr: &Path,
) -> Result<(bool, Buffer), Box<dyn Error>> {
    let buf = Buffer::ansi();
    Ok((!output.status.success(), buf))
}

fn print_diff<'a, W: WriteColor>(mut buf: W, diff: TextDiff<'a, 'a, 'a, str>) {
    // let mut last_lines: ArrayDeque<[_; 3], Wrapping> = ArrayDeque::new();
    let mut multiple_diffs = false;

    for ops in diff.grouped_ops(3) {
        for op in ops {
            for change in diff.iter_changes(&op) {
                let sign = match change.tag() {
                    ChangeTag::Delete => "-",
                    ChangeTag::Insert => "+",
                    ChangeTag::Equal => " ",
                };

                if change.tag() != ChangeTag::Equal {
                    if multiple_diffs {
                        write!(&mut buf, "...").unwrap();
                    }
                    let color = chunk_color(change.tag());
                    buf.set_color(&color).unwrap();
                    let index = change.old_index().or(change.new_index()).unwrap();

                    for line in change.value().lines() {
                        writeln!(&mut buf, "{} {:>2} ┊ {}", sign, index, line).unwrap();
                    }
                    buf.set_color(&ColorSpec::new()).unwrap();
                }
            }
        }
        multiple_diffs = true;
    }
    buf.set_color(&ColorSpec::new()).unwrap();
}

fn chunk_color(chunk: ChangeTag) -> ColorSpec {
    match chunk {
        ChangeTag::Equal => ColorSpec::new(),
        ChangeTag::Delete => {
            let mut c = ColorSpec::new();
            c.set_fg(Some(Color::Red));
            c
        }
        ChangeTag::Insert => {
            let mut c = ColorSpec::new();
            c.set_fg(Some(Color::Green));
            c
        }
    }
}
