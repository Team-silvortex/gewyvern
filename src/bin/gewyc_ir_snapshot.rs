use gewyvern::gewyc::{RenderFormat, compile_ir_report_file, render_ir_history_snapshot};
use std::env;

fn main() {
    let cli = Cli::from_args(env::args().skip(1)).unwrap_or_else(|message| {
        eprintln!("{message}");
        std::process::exit(2);
    });

    let report = compile_ir_report_file(&cli.path).unwrap_or_else(|err| {
        eprintln!("failed to compile ir report: {err:?}");
        std::process::exit(2);
    });

    let rendered = render_ir_history_snapshot(&report, cli.format);
    println!("{rendered}");
}

#[derive(Debug)]
struct Cli {
    path: String,
    format: RenderFormat,
}

impl Cli {
    fn from_args<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut path = None;
        let mut format = RenderFormat::Text;

        for arg in args {
            match arg.as_str() {
                "--json" => format = RenderFormat::Json,
                "--text" => format = RenderFormat::Text,
                "--help" | "-h" => return Err(usage().into()),
                other if other.starts_with('-') => {
                    return Err(format!("unknown option '{other}'\n{}", usage()));
                }
                other => {
                    if path.is_some() {
                        return Err(format!("unexpected extra argument '{other}'\n{}", usage()));
                    }
                    path = Some(other.to_string());
                }
            }
        }

        Ok(Self {
            path: path.ok_or_else(|| usage().to_string())?,
            format,
        })
    }
}

fn usage() -> &'static str {
    "usage: gewyc_ir_snapshot <path.gewy> [--json|--text]"
}
