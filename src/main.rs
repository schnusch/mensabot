use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::env;

extern crate env_logger;
#[macro_use]
extern crate log;
extern crate regex;
extern crate reqwest;
extern crate select;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate time;

use select::predicate::Predicate;

mod conf;
mod levenshtein;
mod tg;

fn time_cmp(x: (i32, i32, i32), y: (u8, u8, u8)) -> i32 {
	let mut r = x.0 - y.0 as i32;
	if r == 0 {
		r = x.1 - y.1 as i32;
		if r == 0 {
			r = x.2 - y.2 as i32;
		}
	}
	r
}

fn parse_tomorrow(tomorrow: &str) -> Result<(u8, u8, u8), String> {
	let topts = tomorrow.split(":").map(|x| x.parse::<u8>()).collect::<Vec<_>>();
	if 2 <= topts.len() && topts.len() <= 3 && topts.iter().all(|x| x.is_ok()) {
		let mut t = topts.into_iter().map(|x| x.unwrap());
		let     t = (t.next().unwrap(), t.next().unwrap(), t.next().unwrap_or(0));
		if time_cmp((24, 0, 0), t.clone()) >= 0 {
			return Ok(t);
		}
	}
	Err(format!("invalid timestamp '{}'", tomorrow))
}

fn strip_white(s: &str) -> String {
	let words = s.split_whitespace();
	let mut s = String::with_capacity(s.len());
	for w in words.into_iter() {
		s.push_str(w);
		s.push_str(" ");
	}
	let n = s.len();
	if n > 0 {
		s.truncate(n - 1);
		s.shrink_to_fit();
	}
	s
}

fn get_text_content(x: &select::node::Node) -> String {
	strip_white(&x.children().filter_map(|x| x.as_text()).collect::<String>())
}

#[derive(Eq)]
struct MensaMatch {
	similarity: usize,
	name:       String
}
impl Ord for MensaMatch {
	fn cmp(&self, other: &MensaMatch) -> Ordering {
		let x = other.similarity.cmp(&self.similarity); // higher similarity first
		match x {
			Ordering::Equal => self.name.cmp(&other.name),
			_               => x
		}
	}
}
impl PartialOrd for MensaMatch {
	fn partial_cmp(&self, other: &MensaMatch) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}
impl PartialEq for MensaMatch {
	fn eq(&self, other: &MensaMatch) -> bool {
		self.similarity == other.similarity && self.name == other.name
	}
}

fn parse_menu(doc: select::document::Document, arg: Option<&str>, mensas: &Vec<String>) -> BTreeMap<MensaMatch, Vec<String>> {
	let mut menu: BTreeMap<MensaMatch, Vec<String>> = BTreeMap::new();
	for table in doc.find(select::predicate::Name("table").and(select::predicate::Class("speiseplan"))) {
		let mut children = table.children();

		// find thead
		let mut theadopt: Option<select::node::Node>;
		loop {
			theadopt = children.next();
			let thead = match theadopt {
				None    => break,
				Some(x) => x
			};
			match thead.name() {
				None    => continue,
				Some(x) => if x != "thead" {
					continue
				}
			};
			break;
		}
		let thead = match theadopt {
			None    => continue,
			Some(x) => x
		};

		// get mensa name
		let th = match thead.find(select::predicate::Name("th")).next() {
			None    => continue,
			Some(x) => x
		};
		let mut mensa = get_text_content(&th);
		if &mensa[..9] == "Angebote " {
			mensa = String::from(&mensa[9..]);
		}
		let mensa = MensaMatch{
			similarity: match arg {
				None    => if mensas.iter().any(|x| x == &mensa) { 0 } else { continue; },
				Some(x) => levenshtein::wordwise_levenshtein(x, &mensa.to_lowercase())
			},
			name: mensa
		};

		// get meals
		let tbodies = children
				.filter(|&x| x.name().map_or(false, |y| y == "tbody"))
				.take(4)
				.enumerate()
				.filter_map(|(i, x)| if i == 0 || i == 3 { Some(x) } else { None });
		let trs = tbodies
				.flat_map(|x| x.children())
				.filter(|&x| x.name().map_or(false, |y| y == "tr"));
		let mut meals = Vec::new();
		for tr in trs {
			let td = match tr.find(select::predicate::Name("td").and(select::predicate::Class("text"))).next() {
				None    => continue,
				Some(x) => x
			};
			let a = match td.find(select::predicate::Name("a")).next() {
				None    => continue,
				Some(x) => x
			};
			let txt = get_text_content(&a);
			meals.push(txt);
		}

		if meals.is_empty() {
			continue;
		}

		menu.insert(mensa, meals);
	}

	menu
}

fn get_menu_url(tomorrow: (u8, u8, u8)) -> &'static str {
	let now  = time::now();
	let vnow = (now.tm_hour, now.tm_min, now.tm_sec);
	if time_cmp(vnow, tomorrow) >= 0 {
		"https://www.studentenwerk-dresden.de/mensen/speiseplan/morgen.html"
	} else {
		"https://www.studentenwerk-dresden.de/mensen/speiseplan/"
	}
}

fn fetch_menu(url: &str, arg: Option<&str>, mensas: &Vec<String>) -> Result<BTreeMap<MensaMatch, Vec<String>>, String> {
	let resp = match reqwest::get(url) {
		Err(e) => return Err(format!("{}", e)),
		Ok(r)  => r
	};
	if resp.status() != reqwest::StatusCode::Ok {
		return Err(format!("HTTP error {}", resp.status()));
	}

	let doc = match select::document::Document::from_read(resp) {
		Err(e) => return Err(format!("{}", e)),
		Ok(d)  => d
	};

	Ok(parse_menu(doc, arg, mensas))
}

fn create_menu_message(menu: &BTreeMap<MensaMatch, Vec<String>>) -> String {
	let mut s = String::new();
	let mut similarity = None;
	for (mensa, meals) in menu.iter() {
		let oldlen = s.len();
		match similarity {
			None       => { similarity = Some(mensa.similarity); },
			Some(prev) => if prev > mensa.similarity { break; }
		};
		s.push_str(&mensa.name);
		for meal in meals {
			s.push_str("\n * ");
			s.push_str(meal);
		}
		s.push_str("\n\n");
		if s.len() > 4093 {
			s.truncate(oldlen);
			s.push_str("...\n\n");
			break;
		}
	}
	let n = s.len();
	if n > 0 {
		s.truncate(n - 2);
	}
	s
}

fn make_menu_text(msg: &tg::Message, arg: Option<&str>, mensas: &Vec<String>, tomorrow: (u8, u8, u8)) -> tg::OutgoingText {
	let url = get_menu_url(tomorrow);
	info!("fetching menu");
	let txt = match fetch_menu(url, arg, mensas) {
		Err(e) => {
			error!("cannot fetch menu: {}", e);
			format!("Speiseplan konnte nicht abgerufen werden!\n{}", url)
		},
		Ok(menu) => create_menu_message(&menu)
	};
	let mut re = msg.reply_text(txt);
	re.disable_notification = true;
	re
}

fn make_about_text(msg: &tg::Message, conf: &conf::Config) -> tg::OutgoingText {
	let mut txt = String::from("<b>Copyright 2017-2018 Schnusch</b>
https://www.github.com/schnusch/mensabot/

access: ");
	txt.push_str(match (conf.allow.is_empty(), conf.deny.is_empty()) {
		(true,  true ) => "public",
		(true,  false) => "blacklist",
		(false, true ) => "whitelist",
		(false, false) => "whitelist, blacklist"
	});
	txt.push_str("\ndefault: ");
	for mensa in conf.general.mensas.iter() {
		txt.push_str("<code>");
		txt.push_str(&mensa.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;"));
		txt.push_str("</code>, ");
	}
	if !conf.general.mensas.is_empty() {
		let n = txt.len();
		txt.truncate(n - 2);
	}
	txt.push_str("\ntomorrow: <code>");
	txt.push_str(&conf.general.tomorrow);
	txt.push_str("</code>");
	if !conf.general.patterns.is_empty() {
		txt.push_str("\npatterns:");
		for pat in conf.general.patterns.iter() {
			txt.push_str("\n <code>");
			txt.push_str(&pat.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;"));
			txt.push_str("</code>");
		}
	}

	let mut re = msg.reply_text(txt);
	re.disable_notification = true;
	re.parse_mode = Some(String::from("html"));
	re
}

fn main() {
	let argv: Vec<String> = env::args().collect();
	if argv.len() > 2 {
		eprintln!("{}: too many arguments", argv[0]);
		std::process::exit(2);
	}

	let conf = argv.get(1).map(String::as_ref).unwrap_or("config.toml");
	let conf = match conf::Config::load(conf) {
		Err(e) => {
			eprintln!("{}: cannot load '{}': {}", argv[0], conf, e);
			std::process::exit(1);
		},
		Ok(c) => c
	};

	let mut initerror = false;

	let tomorrow = match parse_tomorrow(&conf.general.tomorrow) {
		Err(e) => {
			eprintln!("{}: {}", argv[0], e);
			initerror = true;
			(0, 0, 0)
		},
		Ok(t) => t
	};

	let mut patterns: Vec<regex::Regex> = Vec::new();
	for pattern in conf.general.patterns.iter() {
		match regex::Regex::new(&pattern) {
			Err(e) => {
				eprintln!("{}: invalid regular expression {}: {}", argv[0], &pattern, e);
				initerror = true;
			},
			Ok(p) => patterns.push(p)
		};
	}

	match env::var_os("RUST_LOG") {
		None    => env::set_var("RUST_LOG", "info"),
		Some(_) => {}
	};
	match env_logger::init() {
		Err(e) => {
			eprintln!("{}: cannot set up logging: {}", argv[0], e);
			initerror = true;
		},
		Ok(_) => {}
	}

	if initerror {
		std::process::exit(1);
	}

	let mut api = tg::Api::new(&conf.general.token);

	let botname = match conf.general.retry("retrieve bot name", || api.get_me()) {
		Err(_) => None,
		Ok(x)  => x.username
	};

	loop {
		let upds = match conf.general.retry("get updates", || api.get_updates(&vec!["message"])) {
			Err(_) => std::process::exit(1),
			Ok(x)  => x
		};

		let mut latest_update: i64 = 0;
		for upd in upds {
			latest_update = upd.update_id;

			let msg = match upd.message {
				None    => continue,
				Some(m) => m
			};

			if !conf.is_allowed(&msg) {
				match msg.from {
					None    => info!("message {} in {} ignored", msg.message_id, msg.chat),
					Some(u) => info!("message {} from {} in {} ignored", msg.message_id, u, msg.chat)
				};
				continue;
			}

			let text = match msg.text {
				None        => continue,
				Some(ref t) => t
			};

			const CMD_MENSA: u32 = 0x01;
			const CMD_ABOUT: u32 = 0x02;
			let mut cmds: u32 = 0;
			let mut arg_start: usize = 0;
			let mut arg_end:   usize = 0;

			for ent in msg.entities.iter().filter(|ref u| u.entity_type == "bot_command") {
				match ent.extract(&text) {
					Err(e)  => error!("cannot extract entity: {}", e),
					Ok(cmd) => {
						let cmd = match botname {
							None              => &cmd,
							Some(ref botname) => {
								let n = cmd.len();
								let m = botname.len();
								if n > m && &cmd[(n - m - 1)..(n - m)] == "@" && &cmd[(n - m)..] == botname {
									&cmd[..(n - m - 1)]
								} else {
									&cmd
								}
							}
						};
						if cmd == "/mensa" {
							cmds |= CMD_MENSA;
							arg_start = ent.offset + ent.length;
							arg_end   = text.len();
						} else if cmd == "/about" {
							cmds |= CMD_ABOUT;
						} else {
							eprintln!("command: {}", cmd);
						}
					}
				}
			}

			if cmds & CMD_MENSA != 0 {
				// narrow mensa search argument down
				for ent in msg.entities.iter() {
					if arg_start <= ent.offset && ent.offset < arg_end {
						arg_end = ent.offset;
					}
				}
			} else {
				// try text patterns
				for pattern in patterns.iter() {
					if pattern.is_match(&text) {
						cmds |= CMD_MENSA;
						arg_end = 0;
					}
				}
			}

			if cmds == 0 {
				info!("chat {} message {} ignored", msg.chat, msg.message_id);
			} else {
				if cmds & CMD_MENSA != 0 {
					let mut arg = None;
					if arg_start < arg_end {
						// extract mensa search argument
						let argent = tg::MessageEntity{
							entity_type: String::new(),
							offset:      arg_start,
							length:      arg_end - arg_start
						};
						match argent.extract(&text) {
							Err(e) => error!("cannot extract argument: {}", e),
							Ok(x)  => {
								let x = x.trim();
								if x.len() > 0 {
									arg = Some(x.to_lowercase());
								}
							}
						}
					}
					let re = make_menu_text(&msg, arg.as_ref().map(String::as_str), &conf.general.mensas, tomorrow.clone());
					let _  = conf.general.retry("send menu", || api.send_text(&re));
				}
				if cmds & CMD_ABOUT != 0 {
					let re = make_about_text(&msg, &conf);
					let _  = conf.general.retry("send about text", || api.send_text(&re));
				}
			}
		}

		api.set_latest_update(latest_update);
	}
}
