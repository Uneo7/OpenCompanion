use std::io::{BufRead, Write};

use oc_install::awake::KeepAwake;
use oc_install::{Host, Plan};

const CONFIRM_WORD: &str = "WRITE";

#[derive(Default)]
pub struct CliHost {
    awake: Option<KeepAwake>,
}

impl Host for CliHost {
    fn log(&mut self, line: &str) {
        println!("{line}");
    }

    fn confirm(&mut self, plan: &Plan) -> bool {
        println!();
        println!("About to write:");
        for line in plan.lines() {
            println!("{line}");
        }
        print!("Type {CONFIRM_WORD} to continue, anything else cancels: ");
        let _ = std::io::stdout().flush();
        let mut answer = String::new();
        if std::io::stdin().lock().read_line(&mut answer).is_err() {
            return false;
        }
        answer.trim() == CONFIRM_WORD
    }

    fn keep_awake(&mut self, on: bool) {
        if !on {
            self.awake = None;
            return;
        }
        match KeepAwake::acquire("OpenCompanion is writing the speaker's EEPROM") {
            Ok(g) => self.awake = Some(g),
            Err(e) => eprintln!(
                "WARNING: cannot keep the computer awake ({e}). Disable sleep / screen-saver suspend manually and do \
                 not let the computer sleep until the end."
            ),
        }
    }
}
