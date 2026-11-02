use std::path::PathBuf;

use anyhow::{bail, Context};
use clap::{Subcommand, ValueEnum};
use oc_device::base::json::{self, Form, Options};
use oc_device::base::{convert, payload, topology, Base};

#[derive(Clone, Copy, ValueEnum)]
pub enum FormArg {
    /// Exact float32 coefficients.
    Raw,
    /// Poles and zeros (readable; one rounding when read back).
    Zpk,
}

#[derive(Subcommand)]
pub enum BaseCmd {
    /// Convert the stock program of a stock DSP backup into a base file ("Bose d'origine"). Offline.
    Export {
        /// 64 KB U7001 dump holding the stock firmware (default: the oldest verified stock backup of the app).
        #[arg(long)]
        backup: Option<PathBuf>,
        /// Output file (default: standard output).
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "raw")]
        form: FormArg,
        /// Leave out the `about` annotations.
        #[arg(long)]
        no_about: bool,
    },
    /// Check a base file: format, hard rules, encoded size, difference from the stock base. Offline.
    Check {
        file: PathBuf,
        /// Stock backup to compare with (default: the app's, if any).
        #[arg(long)]
        backup: Option<PathBuf>,
    },
}

fn stock(backup: Option<PathBuf>) -> anyhow::Result<Base> {
    Ok(match backup {
        Some(p) => convert::from_stock_dump(&std::fs::read(&p).with_context(|| p.display().to_string())?)?,
        None => convert::from_backups(&oc_install::backup::default_root())?,
    })
}

pub fn run(cmd: BaseCmd) -> anyhow::Result<()> {
    match cmd {
        BaseCmd::Export { backup, output, form, no_about } => {
            let b = stock(backup)?;
            let form = match form {
                FormArg::Raw => Form::Raw,
                FormArg::Zpk => Form::Zpk,
            };
            let text = json::export(&b, Options { form, annotate: !no_about });
            match output {
                Some(p) => {
                    std::fs::write(&p, text).with_context(|| p.display().to_string())?;
                    eprintln!("written: {} ({}, {} payload bytes)", p.display(), topology::NAME, topology::payload_len());
                }
                None => print!("{text}"),
            }
        }
        BaseCmd::Check { file, backup } => {
            let text = std::fs::read_to_string(&file).with_context(|| file.display().to_string())?;
            let b = match json::import(&text) {
                Ok(b) => b,
                Err(e) => bail!("{}: {e}", file.display()),
            };
            let bytes = payload::encode(&b, 0)?;
            println!("{}: valid, topology {}, {} of {} payload bytes", file.display(), topology::NAME, bytes.len() - payload::HDR_LEN, payload::CAPACITY);
            match stock(backup) {
                Ok(s) => {
                    let (a, z) = (b.floats(), s.floats());
                    let diff = a.iter().zip(&z).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
                    let dl = b.delays.iter().zip(&s.delays).filter(|(x, y)| x != y).count();
                    if diff == 0 && dl == 0 {
                        println!("identical to the stock base (bit for bit)");
                    } else {
                        println!("differs from the stock base: {diff} of {} values, {dl} delays", a.len());
                    }
                }
                Err(e) => println!("no stock comparison ({e})"),
            }
            println!("note: the response-based safety rules (spec 4.2) are not checked yet");
        }
    }
    Ok(())
}
