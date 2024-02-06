use std::collections::HashMap;
use std::io;
use std::process;

use clap::{Arg, ArgMatches, Command};
use mdbook::book::Book;
use mdbook::errors::Error;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor, PreprocessorContext};
use semver::{Version, VersionReq};

use crate::pandoc_lib::Pandoc;

pub fn make_app() -> Command {
	Command::new("citeproc-preprocessor")
			.about("A mdbook preprocessor which runs your code through pandoc and citeproc")
			.subcommand(
				Command::new("supports")
						.arg(Arg::new("renderer").required(true))
						.about("Check whether a renderer is supported by this preprocessor"),
			)
}

fn main() {
	let matches = make_app().get_matches();

	let preprocessor = Pandoc::new();

	if let Some(sub_args) = matches.subcommand_matches("supports") {
		handle_supports(&preprocessor, sub_args);
	} else if let Err(e) = handle_preprocessing(&preprocessor) {
		eprintln!("{}", e);
		process::exit(1);
	}
}

fn handle_preprocessing(pre: &dyn Preprocessor) -> Result<(), Error> {
	let (ctx, book) = CmdPreprocessor::parse_input(io::stdin())?;

	let book_version = Version::parse(&ctx.mdbook_version)?;
	let version_req = VersionReq::parse(mdbook::MDBOOK_VERSION)?;

	if !version_req.matches(&book_version) {
		eprintln!(
			"Warning: The {} plugin was built against version {} of mdbook, \
             but we're being called from version {}",
			pre.name(),
			mdbook::MDBOOK_VERSION,
			ctx.mdbook_version
		);
	}

	let processed_book = pre.run(&ctx, book)?;
	serde_json::to_writer(io::stdout(), &processed_book)?;

	Ok(())
}

fn handle_supports(pre: &dyn Preprocessor, sub_args: &ArgMatches) -> ! {
	let renderer = sub_args
			.get_one::<String>("renderer")
			.expect("Required argument");
	let supported = pre.supports_renderer(renderer);

	// Signal whether the renderer is supported by exiting with 1 or 0.
	if supported {
		process::exit(0);
	} else {
		process::exit(1);
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PandocSetting {
	Preserve,
	Transpile,
}

impl Default for PandocSetting {
	fn default() -> Self {
		Self::Preserve
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BibliographyConfig {
	bibliography: String,
	bibliography_style: String,
}

impl BibliographyConfig {
	fn new(bibliography: String, bibliography_style: String) -> Self {
		Self {
			bibliography,
			bibliography_style,
		}
	}
}


type PandocConfig = HashMap<String, PandocSetting>;


/// The actual implementation of the `Pandoc` preprocessor. This would usually go
/// in your main `lib.rs` file.
mod pandoc_lib {
	use std::io::Write;

	use mdbook::BookItem;

	use super::*;

	pub struct Pandoc;

	impl Pandoc {
		pub fn new() -> Self {
			Self
		}
	}

	impl Preprocessor for Pandoc {
		fn name(&self) -> &str {
			"citeproc"
		}

		fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
			let res: Option<Result<BookItem, Error>> = None;
			let mut config: PandocConfig = HashMap::new();

			let mut from = "--from=markdown_strict".to_string();
			let mut to = "--to=markdown_strict".to_string();

			let bibliography_config;

			if let Some(table) = ctx.config.get_preprocessor(self.name()) {
				let mut parse_setting = |setting: &String, config: &mut PandocConfig| {
					if let Some(option) = table.get(setting) {
						from += &format!("+{}", setting.as_str()).to_string();
						let new_value = match option.as_str() {
							Some("transpile") => {
								to += &format!("-{}", setting.as_str()).to_string();
								PandocSetting::Transpile
							}
							Some("preserve") => {
								to += &format!("+{}", setting.as_str()).to_string();
								PandocSetting::Preserve
							}
							None => {
								to += &format!("+{}", setting.as_str()).to_string();
								PandocSetting::default()
							}
							Some(_) => panic!(
								"{} must be either \"transpile\" or \"preserve\"", setting
							)
						};
						if let Some(value) = config.get_mut(setting) {
							*value = new_value;
						} else {
							config.insert(setting.clone(), new_value);
						}
					};
				};

				parse_setting(&"backtick_code_blocks".to_string(), &mut config);
				parse_setting(&"bracketed_spans".to_string(), &mut config);
				parse_setting(&"citations".to_string(), &mut config);
				parse_setting(&"definition_lists".to_string(), &mut config);
				parse_setting(&"emoji".to_string(), &mut config);
				parse_setting(&"fenced_code_attributes".to_string(), &mut config);
				parse_setting(&"fenced_code_blocks".to_string(), &mut config);
				parse_setting(&"fenced_divs".to_string(), &mut config);
				parse_setting(&"footnotes".to_string(), &mut config);
				parse_setting(&"hard_line_breaks".to_string(), &mut config);
				parse_setting(&"inline_notes".to_string(), &mut config);
				parse_setting(&"mark".to_string(), &mut config);
				parse_setting(&"markdown_in_html_blocks".to_string(), &mut config);
				parse_setting(&"link_attributes".to_string(), &mut config);


				let config = config;

				bibliography_config = if let Some(PandocSetting::Transpile) = config.get("citations") {
					if let (Some(bib_style), Some(bib)) = (
						table.get("bibliography-style").and_then(|x| x.as_str()),
						table.get("bibliography").and_then(|x| x.as_str()),
					) {
						Some(BibliographyConfig::new(
							bib.to_string(),
							bib_style.to_string(),
						))
					} else {
						panic!("citations set to transpile so bibliography-style and bibliography option must be provided!")
					}
				} else {
					None
				};
			} else {
				panic!("No config table for {} preprocessor", self.name());
			}

			book.for_each_mut(|item| {
				if let Some(Err(_)) = res {
					return;
				}
				if let BookItem::Chapter(ref mut chapter) = *item {
					let mut process = process::Command::new("pandoc");
					let command = process.arg(from.clone()).arg(to.clone());
					let command = if let Some(bibliography_config) = &bibliography_config {
						command
								.arg(format!("--csl={}", bibliography_config.bibliography_style))
								.arg(format!("--bibliography={}", bibliography_config.bibliography))
								.arg("--metadata=link-citations")
								.arg("--metadata=link-bibliography")
								.arg("--citeproc")
					} else {
						command
					};
					let mut process = command
							.stdin(process::Stdio::piped())
							.stdout(process::Stdio::piped())
							.spawn()
							.expect("failed to spawn process");
					process
							.stdin
							.take()
							.expect("failed to open pandoc stdin")
							.write_all(chapter.content.as_bytes())
							.expect("failed to write to pandoc stdin");
					let output = process.wait_with_output().expect("failed to wait on pandoc");
					chapter.content = String::from_utf8_lossy(
						output.stdout.as_slice()
					).to_string();
				}
			});

			Ok(book)
		}

		fn supports_renderer(&self, renderer: &str) -> bool {
			renderer != "not-supported"
		}
	}
}
