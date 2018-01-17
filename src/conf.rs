use std::collections::BTreeSet;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::thread;
use std::time::Duration;

extern crate toml;

use tg;

#[derive(Deserialize)]
pub struct ConfigGeneral {
	pub token:     String,
	#[serde(default="ConfigGeneral::default_tomorrow")]
	pub tomorrow:  String,
	#[serde(default="ConfigGeneral::default_retries")]
	pub retries:   u64,
	#[serde(default="ConfigGeneral::default_retrywait")]
	pub retrywait: u64,
	#[serde(default="ConfigGeneral::default_mensas")]
	pub mensas:    Vec<String>,
	#[serde(default)]
	pub patterns:  Vec<String>
}
impl ConfigGeneral {
	fn default_tomorrow() -> String {
		String::from("20:00:00")
	}

	fn default_retries() -> u64 {
		3
	}

	fn default_retrywait() -> u64 {
		30
	}

	fn default_mensas() -> Vec<String> {
		vec![
			String::from("Alte Mensa"),
			String::from("Zeltschlösschen")
		]
	}

	pub fn retry<E, F, R>(&self, msg: &str, mut action: F) -> Result<R, ()>
			where E: fmt::Display, F: FnMut() -> Result<R, E> {
		let mut fails = 0;
		loop {
			match action() {
				Err(e) => {
					fails += 1;
					if fails >= self.retries && self.retries > 0 {
						error!("cannot {} (try {}/{}): {}", msg, fails, self.retries, e);
						return Err(());
					} else if self.retrywait == 0 {
						warn!("cannot {}: {}, retrying...", msg, e);
						fails -= 1; // do not count fails
					} else {
						warn!("cannot {} (try {}/{}): {}, retrying in {} seconds...", msg, fails, self.retries, e, self.retrywait);
						thread::sleep(Duration::from_secs(self.retrywait));
					}
				},
				Ok(x) => return Ok(x)
			}
		}
	}
}

#[derive(Deserialize)]
pub struct ConfigAccess {
	#[serde(rename="chats", default)]
	chatids:   BTreeSet<i64>,
	#[serde(rename="users", default)]
	usernames: BTreeSet<String>,
	#[serde(skip)]
	userids:   BTreeSet<i64>
}
impl ConfigAccess {
	fn new() -> ConfigAccess {
		ConfigAccess{
			chatids:   BTreeSet::new(),
			usernames: BTreeSet::new(),
			userids:   BTreeSet::new()
		}
	}

	fn unpack(&mut self) {
		for ref user in self.usernames.clone() {
			match user.parse::<i64>() {
				Ok(id) => {
					self.userids.replace(id);
					self.usernames.remove(user);
				},
				Err(_) => if &user[..1] == "@" {
					self.usernames.replace(String::from(&user[1..]));
					self.usernames.remove(user);
				}
			}
		}
	}

	fn contains_user(&self, user: Option<&tg::User>) -> bool {
		match user {
			None    => false,
			Some(u) => self.userids.contains(&u.id) || match u.username {
				None           => false,
				Some(ref name) => self.usernames.contains(name)
			}
		}
	}

	pub fn is_empty(&self) -> bool {
		self.chatids.is_empty() && self.userids.is_empty() && self.usernames.is_empty()
	}
}

#[derive(Deserialize)]
pub struct Config {
	pub general: ConfigGeneral,
	#[serde(default="ConfigAccess::new")]
	pub allow: ConfigAccess,
	#[serde(default="ConfigAccess::new")]
	pub deny: ConfigAccess
}
impl Config {
	pub fn load<P: AsRef<Path>>(name: P) -> Result<Config, String> where P: fmt::Display {
		let mut f = File::open(&name).map_err(|e| format!("cannot open `{}`: {}", name, e.to_string()))?;
		let mut s = String::new();
		f.read_to_string(&mut s).map_err(|e| format!("cannot read `{}`: {}", name, e.to_string()))?;
		let mut conf: Config = toml::from_str(&s).map_err(|e| format!("cannot load `{}`: {}", name, e.to_string()))?;
		conf.allow.unpack();
		conf.deny.unpack();
		if conf.general.mensas.is_empty() {
			conf.general.mensas = ConfigGeneral::default_mensas();
		}
		Ok(conf)
	}

	pub fn is_allowed(&self, msg: &tg::Message) -> bool {
		if self.allow.contains_user(msg.from.as_ref()) {
			true
		} else if self.deny.contains_user(msg.from.as_ref()) {
			false
		} else if self.allow.chatids.contains(&msg.chat.id) {
			true
		} else if self.deny.chatids.contains(&msg.chat.id) {
			false
		} else if self.allow.is_empty() {
			true
		} else {
			false
		}
	}
}
