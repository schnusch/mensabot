use std::cmp::max;
use std::cmp::min;
use std::cmp::Ordering;
use std::fmt;
use std::iter;

trait VecReplace<T> {
	fn replace(&mut self, i: usize, new: T) -> T;
}
impl<T> VecReplace<T> for Vec<T> {
	fn replace(&mut self, i: usize, new: T) -> T {
		self.push(new);
		self.swap_remove(i)
	}
}

const KEEP:  u8 = 0;
const SUBST: u8 = 1;
const DEL:   u8 = 2;
const INS:   u8 = 3;

pub enum Operation {
	Keep,
	Subst,
	Insert,
	Delete
}
impl Operation {
	pub fn is_keep(&self) -> bool {
		match self {
			&Operation::Keep => true,
			_                => false
		}
	}
}

pub struct Distance {
	pub distance: usize,
	words: Vec<u64>,
	len:   usize
}
impl Distance {
	fn new() -> Distance {
		Distance {
			distance: 0,
			words:    Vec::new(),
			len:      0
		}
	}

	fn next(&self, op: Operation) -> Distance {
		let opcode = match op {
			Operation::Keep   => KEEP,
			Operation::Subst  => SUBST,
			Operation::Insert => INS,
			Operation::Delete => DEL
		} as u64;
		let mut newwords = self.words.to_vec();
		if self.len % 32 == 0 {
			newwords.push(opcode);
		} else {
			let n = newwords.len();
			newwords[n - 1] |= opcode << (self.len % 32 * 2);
		}
		Distance {
			distance: self.distance + !op.is_keep() as usize,
			words:    newwords,
			len:      self.len + 1
		}
	}

	fn bits_to_operation(x: u64) -> Operation {
		match x as u8 {
			KEEP  => Operation::Keep,
			SUBST => Operation::Subst,
			INS   => Operation::Insert,
			DEL   => Operation::Delete,
			_     => unreachable!()
		}
	}

	fn get_last_operation(&self) -> Operation {
		if self.len == 0 {
			panic!("levenshtein distance empty");
		}
		let n  = self.len - 1;
		let iw = n / 32;
		let ib = n % 32;
		Distance::bits_to_operation((self.words[iw] >> (ib * 2)) & 3)
	}

	fn iter(&self) -> Iterator {
		Iterator {
			lev: self,
			i:   0
		}
	}
}
impl Ord for Distance {
	fn cmp(&self, other: &Distance) -> Ordering {
		self.distance.cmp(&other.distance)
	}
}
impl PartialOrd for Distance {
	fn partial_cmp(&self, other: &Distance) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}
impl PartialEq for Distance {
	fn eq(&self, other: &Distance) -> bool {
		self.distance == other.distance && self.len == other.len && self.words == other.words
	}
}
impl Eq for Distance {}
impl fmt::Debug for Distance {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		for a in self.iter() {
			let c = match a {
				Operation::Keep   => "=",
				Operation::Subst  => "!",
				Operation::Insert => "+",
				Operation::Delete => "-"
			};
			write!(f, "{}", c)?;
		}
		Ok(())
	}
}

pub struct Iterator<'a> {
	lev: &'a Distance,
	i:   usize
}
impl<'a> iter::Iterator for Iterator<'a> {
	type Item = Operation;
	fn next(&mut self) -> Option<Self::Item> {
		if self.i >= self.lev.len {
			return None;
		}
		let iw = self.i / 32;
		let ib = self.i % 32;
		self.i += 1;
		Some(Distance::bits_to_operation((self.lev.words[iw] >> (ib * 2)) & 3))
	}
}

pub fn levenshtein(a: &str, b: &str) -> Distance {
	let mut lev: Vec<Distance> = Vec::with_capacity(b.len() + 1); // one more to use swap_remove

	let n = {
		lev.push(Distance::new());
		let mut j = 0;
		for _ in b.chars() {
			let next = lev[j].next(Operation::Insert);
			lev.push(next);
			j += 1;
		}
		j
	};

	lev.shrink_to_fit();

	if cfg!(debug_assertions) {
		eprint!("    ");
		for cb in b.chars() {
			eprint!("  {}", cb);
		}
		eprint!("\n   0");
		for i in lev.iter().skip(1) {
			eprint!(" \x1b[1;32m{:>2}\x1b[22;39m", i.distance);
		}
		eprint!("\n");
	}

	for ca in a.chars() {
		let     new   = lev[0].next(Operation::Delete);
		let mut lev11 = lev.replace(0, new);

		if cfg!(debug_assertions) {
			eprint!("{} \x1b[1;31m{:>2}\x1b[22;39m", ca, lev[0].distance);
		}

		for (j, cb) in b.chars().enumerate() {
			let j = j + 1;
			let insert = lev[j - 1].next(Operation::Insert);
			let delete = lev[  j  ].next(Operation::Delete);
			let subst  = lev11.next(if ca == cb { Operation::Keep } else { Operation::Subst });
			lev11 = lev.replace(j, min(min(insert, delete), subst));

			if cfg!(debug_assertions) {
				let color = match lev[j].get_last_operation() {
					Operation::Keep   => "22;39",
					Operation::Subst  => "1;33",
					Operation::Insert => "1;32",
					Operation::Delete => "1;31"
				};
				eprint!(" \x1b[{}m{:>2}\x1b[22;39m", color, lev[j].distance);
			}
		}

		if cfg!(debug_assertions) {
			eprint!("\n");
		}
	}

	let lev = lev.remove(n);

	if cfg!(debug_assertions) {
		eprintln!("{} -> {} = {:?}", a, b, lev);
	}

	return lev;
}

fn find_best_word_match(mut rows: &mut Vec<Vec<usize>>) -> usize {
	let mut max = 0;
	let row = match rows.pop() {
		None    => return max,
		Some(r) => r
	};
	let numrows = rows.len();
	for (i, d) in row.iter().enumerate() {
		// remove column i
		let mut col = Vec::with_capacity(numrows);
		for row2 in rows.iter_mut() {
			col.push(row2.remove(i));
		}
		// go deeper
		let d2 = find_best_word_match(rows);
		// reinsert column i
		for (j, y) in col.into_iter().enumerate() {
			rows[j].insert(i, y);
		}

		let d = d + d2;
		if d > max {
			max = d;
		}
	}
	rows.push(row);
	max
}

pub fn wordwise_levenshtein(a: &str, b: &str) -> usize {
	struct LenStr<'a> {
		len: usize,
		s:   &'a str
	}
	struct Words<'a> {
		a: Vec<LenStr<'a>>,
		b: Vec<LenStr<'a>>,
		anum: usize,
		bnum: usize
	}

	fn split(x: &str) -> Vec<LenStr> {
		x.split_whitespace()
				.flat_map(|y| y.split(|c| c == '(' || c == ')' || c == ':'))
				.filter_map(|z| if z.len() > 0 { Some(LenStr{len: z.chars().count(), s: z}) } else { None })
				.collect()
	}

	let words = {
		let mut ws = Words {
			a: split(a),
			b: split(b),
			anum: 0,
			bnum: 0
		};
		ws.anum = ws.a.len();
		ws.bnum = ws.b.len();
		ws
	};
	let maxwordnum = max(words.anum, words.bnum);

	let mut d: Vec<Vec<usize>> = Vec::with_capacity(maxwordnum);
	for aword in words.a.iter() {
		let mut row = Vec::with_capacity(maxwordnum);
		for bword in words.b.iter() {
			row.push(max(aword.len, bword.len) - levenshtein(aword.s, bword.s).distance);
		}

		// anum > bnum -> add columns
		for _ in words.bnum..maxwordnum {
			row.push(0);
		}

		d.push(row);
	}

	// bnum > anum -> add rows
	for _ in words.anum..maxwordnum {
		let mut row = Vec::with_capacity(maxwordnum);
		for _ in words.b.iter() {
			row.push(0);
		}
		d.push(row);
	}

	find_best_word_match(&mut d)
}
