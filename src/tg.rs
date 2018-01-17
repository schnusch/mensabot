use std;
use std::error;
use std::fmt;
use std::string::FromUtf16Error;

extern crate reqwest;
extern crate serde;
extern crate serde_json;

#[derive(Debug)]
pub struct Error {
	desc: String
}
impl Error {
	fn new<S: Into<String>>(desc: S) -> Error {
		Error { desc: desc.into() }
	}
}
impl error::Error for Error {
	fn description(&self) -> &str {
		&self.desc
	}
}
impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.desc)
	}
}

#[derive(Deserialize)]
struct Response {
	ok:          bool,
	error_code:  Option<i64>,
	description: Option<String>,
	result:      Option<serde_json::Value>
}

#[derive(Deserialize, Debug)]
pub struct Chat {
	pub id:         i64,
	pub title:      Option<String>,
	pub username:   Option<String>,
	pub first_name: Option<String>,
	pub last_name:  Option<String>
}
impl fmt::Display for Chat {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self.title {
			None        => User::fmt_name(self.first_name.as_ref(), self.username.as_ref(), self.last_name.as_ref(), f),
			Some(ref t) => write!(f, "{}", t)
		}
	}
}

#[derive(Deserialize, Debug)]
pub struct User {
	pub id:         i64,
	pub first_name: String,
	pub last_name:  Option<String>,
	pub username:   Option<String>
}
impl User {
	fn fmt_name(firstname: Option<&std::string::String>, username: Option<&std::string::String>,
			lastname: Option<&std::string::String>, fmt: &mut fmt::Formatter) -> fmt::Result {
		let pad = match firstname {
			None    => "",
			Some(f) => {fmt.write_str(f)?; " "}
		};
		let pad = match username {
			None    => pad,
			Some(u) => {write!(fmt, "{}'{}'", pad, u)?; " "}
		};
		match lastname {
			None    => if pad.len() == 0 { fmt.write_str("<unknown>")? },
			Some(l) => write!(fmt, "{}{}", pad, l)?
		};
		Ok(())
	}
}
impl fmt::Display for User {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		User::fmt_name(Some(&self.first_name), self.username.as_ref(), self.last_name.as_ref(), f)
	}
}

#[derive(Deserialize, Debug)]
pub struct MessageEntity {
	#[serde(rename="type")]
	pub entity_type: String,
	pub offset:      usize,
	pub length:      usize
}
impl MessageEntity {
	// TODO extract multiple at once with iterators
	pub fn extract(&self, text: &str) -> Result<String, FromUtf16Error> {
		String::from_utf16(&text.encode_utf16().skip(self.offset).take(self.length).collect::<Vec<u16>>())
	}
}

#[derive(Deserialize, Debug)]
pub struct Message {
	pub message_id: i64,
	pub chat:       Chat,
	pub from:       Option<User>,
	pub text:       Option<String>,
	#[serde(default)]
	pub entities:   Vec<MessageEntity>
}
impl Message {
	pub fn reply_text<S: Into<String>>(&self, text: S) -> OutgoingText {
		OutgoingText {
			chat_id:    self.chat.id,
			text:       text.into(),
			parse_mode: None,
			disable_notification: false,
			reply_to_message_id:  Some(self.message_id)
		}
	}
}

#[derive(Deserialize)]
pub struct Update {
	pub update_id: i64,
	pub message:   Option<Message>
}

#[derive(Serialize)]
struct UpdateRequest<'a> {
	offset:          i64,
	timeout:         i64,
	#[serde(skip_serializing_if="Vec::is_empty")]
	allowed_updates: &'a Vec<&'a str>
}

#[derive(Serialize)]
pub struct OutgoingText {
	pub chat_id:              i64,
	pub text:                 String,
	#[serde(skip_serializing_if="OutgoingText::is_true")]
	pub disable_notification: bool,
	#[serde(skip_serializing_if="Option::is_none")]
	pub parse_mode:           Option<String>,
	#[serde(skip_serializing_if="Option::is_none")]
	pub reply_to_message_id:  Option<i64>
}
impl OutgoingText {
	fn is_true(b: &bool) -> bool {
		*b
	}
}

pub struct Api {
	baseurl: String,
	client:  reqwest::Client,
	offset:  i64
}
impl Api {
	pub fn new(token: &str) -> Api {
		Api {
			baseurl:   format!("https://api.telegram.org/bot{}/", token),
			client:    reqwest::Client::new(),
			offset:    0
		}
	}

	fn get_result<T>(resp: Response) -> Result<T, Error> where for<'de> T: serde::Deserialize<'de> {
		if resp.ok {
			match resp.result {
				None    => Err(Error::new(format!("unexpected JSON response"))),
				Some(r) => match serde_json::from_value::<T>(r) {
					Err(_) => Err(Error::new("unexpected JSON result")),
					Ok(x)  => Ok(x)
				}
			}
		} else {
			Err(Error::new(if resp.error_code.is_some() && resp.description.is_some() {
				format!("Telegram Bot API Error: {} {}",
						resp.error_code.unwrap(), resp.description.unwrap())
			} else {
				format!("unexpected JSON response")
			}))
		}
	}

	fn api_call<D, T>(&self, method: &str, data: &D) -> Result<T, Error>
			where D: serde::Serialize, for<'de> T: serde::Deserialize<'de> {
		let resp = self.client.post(&format!("{}{}", self.baseurl, method))
				.header(reqwest::header::ContentType::json())
				.json(data)
				.send();
		match resp {
			Err(e)     => Err(Error::new(format!("reqwest error: {}", e))),
			Ok(mut re) => if re.status().is_success() {
				match re.json() {
					Err(e) => Err(Error::new(format!("deserialization error: {}", e))),
					Ok(r)  => Api::get_result(r)
				}
			} else {
				Err(Error::new(format!("Telegram Bot API HTTP error: {}", re.status())))
			}
		}
	}

	pub fn get_me(&self) -> Result<User, Error> {
		self.api_call("getMe", &())
	}

	pub fn get_updates(&mut self, allowed_updates: &Vec<&str>) -> Result<Vec<Update>, Error> {
		let req = UpdateRequest {
			offset:          self.offset,
			timeout:         30,
			allowed_updates: allowed_updates
		};
		self.api_call("getUpdates", &req)
	}

	pub fn send_text(&self, msg: &OutgoingText) -> Result<Message, Error> {
		return self.api_call("sendMessage", msg);
	}

	pub fn set_latest_update(&mut self, latest_update: i64) {
		self.offset = latest_update + 1;
	}
}
