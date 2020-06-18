//! `generate` subcommand - generates a skeleton config.

use crate::config::ZebradConfig;
use abscissa_core::{Command, Options, Runnable};

/// `generate` subcommand
#[derive(Command, Debug, Options)]
pub struct GenerateCmd {
    /// The file to write the generated config to.
    #[options(help = "The file to write the generated config to (stdout if unspecified)")]
    output_file: Option<String>,
}

impl Runnable for GenerateCmd {
    /// Start the application.
    fn run(&self) {
        let default_config = ZebradConfig {
            tracing: crate::config::TracingSection::populated(),
            network: Default::default(),
            metrics: Default::default(),
        };
        let mut output = r"# Default configuration values for zebrad.
#
# This file is intended as a skeleton for custom configs.
#
# Because this contains default values, and the default
# values may change, you should delete all entries except
# for the ones you wish to change.
#
# Documentation on the meanings of each config field
# can be found in Rustdoc here:
# https://doc.zebra.zfnd.org/zebrad/config/struct.ZebradConfig.html

"
        .to_owned(); // The default name and location of the config file is defined in ../commands.rs

        // this avoids a ValueAfterTable error
        // https://github.com/alexcrichton/toml-rs/issues/145
        let conf = toml::Value::try_from(default_config).unwrap();
        output += &toml::to_string_pretty(&conf).expect("default config should be serializable");
        match self.output_file {
            Some(ref output_file) => {
                use std::{fs::File, io::Write};
                File::create(output_file)
                    .expect("must be able to open output file")
                    .write_all(output.as_bytes())
                    .expect("must be able to write output");
            }
            None => {
                println!("{}", output);
            }
        }
    }
}