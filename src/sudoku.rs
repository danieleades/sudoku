use rand::Rng;

use consts::*;
use types::{Entry, PubEntry, BlockFormatParseError, LineFormatParseError, NotEnoughRows};
use solver::SudokuSolver;
use generator::SudokuGenerator;

use std::{fmt, slice, iter, hash, cmp, ops::{self, Deref}, str};
#[cfg(feature="serde")] use ::serde::{de, Serialize, Serializer, Deserialize, Deserializer};

/// The main structure exposing all the functionality of the library
///
/// `Sudoku"s can generated, constructed from arrays or parsed from `&str`s
/// in either the line or block format.
#[derive(Copy, Clone)]
pub struct Sudoku(pub(crate) [u8; 81]);

#[cfg(feature="serde")]
impl Serialize for Sudoku {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
	{
		if serializer.is_human_readable() {
			serializer.serialize_str(&self.to_str_line())
		} else {
			serializer.serialize_bytes(&self.0)
		}
	}
}

// Visitors for serde
#[cfg(feature="serde")] struct ByteSudoku; // 81 byte format
#[cfg(feature="serde")] struct StrSudoku;  // 81 char format (line sudoku)

#[cfg(feature="serde")]
impl<'de> de::Visitor<'de> for ByteSudoku {
	type Value = Sudoku;
	fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		write!(formatter, "81 numbers from 0 to 9 inclusive")
	}

	fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
	where
    	E: de::Error,
	{
		// FIXME: return proper error
		Sudoku::from_bytes_slice(v).map_err(|_| {
			E::custom("byte array has incorrect length or contains numbers not from 0 to 9")
		})
	}
}

#[cfg(feature="serde")]
impl<'de> de::Visitor<'de> for StrSudoku {
	type Value = Sudoku;
	fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		write!(formatter, "81 numbers from 0 to 9 inclusive")
	}

	fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
	where
    	E: de::Error,
	{
		Sudoku::from_str_line(v).map_err(E::custom)
	}
}

#[cfg(feature="serde")]
impl<'de> Deserialize<'de> for Sudoku {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>
	{
		if deserializer.is_human_readable() {
			deserializer.deserialize_str(StrSudoku)
		} else {
			deserializer.deserialize_bytes(ByteSudoku)
		}
	}
}

impl PartialEq for Sudoku {
	fn eq(&self, other: &Sudoku) -> bool {
		self.0[..] == other.0[..]
	}
}

/// The ordering is lexicographical in the cells of the sudoku
/// going from left to right, top to bottom
impl PartialOrd for Sudoku {
	fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
		// deref into &str and cmp
		self.0.partial_cmp(&other.0)
	}
}

impl Ord for Sudoku {
	fn cmp(&self, other: &Self) -> cmp::Ordering {
		// deref into &str and cmp
		self.0.cmp(&other.0)
	}
}

impl hash::Hash for Sudoku {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher
	{
		self.0.hash(state)
	}
}

impl Eq for Sudoku {}

impl fmt::Debug for Sudoku {
	fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
		self.0.fmt(fmt)
	}
}

type R = ::rand::rngs::ThreadRng;
pub type Iter<'a> = iter::Map<slice::Iter<'a, u8>, fn(&u8)->Option<u8>>; // Iter over Sudoku cells

impl Sudoku {
	/// Generate a random, solved sudoku
	pub fn generate_filled() -> Self {
		SudokuGenerator::generate_filled()
	}

	/// Generate a random, uniquely solvable sudoku
	/// The puzzles are minimal in that no cell can be removed without losing uniquess of solution
	/// Most puzzles generated by this are easy
	pub fn generate_unique() -> Self {
		Sudoku::generate_unique_from( Sudoku::generate_filled() )
	}

	/// Generate a random, uniqely solvable sudoku
	/// that has the same solution as the given `sudoku` by removing the contents of some of its cells.
	/// The puzzles are minimal in that no cell can be removed without losing uniquess of solution.
	/// Most puzzles generated by this from solved sudokus are easy.
	///
	/// If the source `sudoku` is invalid or has multiple solutions, it will be returned as is.
	pub fn generate_unique_from(mut sudoku: Sudoku) -> Self {
		// this function is following
		// the approach outlined here: https://stackoverflow.com/a/7280517
		//
		// delete numbers from a filled sudoku cells in random order
		// after each deletion check for unique solvability
		// and backtrack on error

		// generate random order
		let mut cell_order = [0; 81];
		cell_order.iter_mut()
			.enumerate()
			.for_each(|(cell, place)| *place = cell);
		::rand::thread_rng().shuffle(&mut cell_order);

		// remove cell content if possible without destroying uniqueness of solution
		const CUTOFF: usize = 20;
		let mut sudoku_tmp = sudoku;
		for &cell in &cell_order[..CUTOFF] {
			sudoku_tmp.0[cell] = 0;
		}
		if sudoku_tmp.is_uniquely_solvable() {
			sudoku = sudoku_tmp;
		} else {
			for &cell in &cell_order[..CUTOFF] {
				let mut sudoku_tmp = sudoku;
				sudoku_tmp.0[cell] = 0;
				if sudoku_tmp.is_uniquely_solvable() {
					sudoku = sudoku_tmp;
				}
			}
		}

		let mut n_cell = CUTOFF;
		while n_cell < 50 {
			let mut sudoku_tmp = sudoku;
			let cell1 = cell_order[n_cell];
			let cell2 = cell_order[n_cell+1];
			sudoku_tmp.0[cell1] = 0;
			sudoku_tmp.0[cell2] = 0;
			if sudoku_tmp.is_uniquely_solvable() {
				// both numbers can be left out
				sudoku = sudoku_tmp;
				n_cell += 2;
				continue
			}

			sudoku_tmp.0[cell2] = sudoku.0[cell2];
			if sudoku_tmp.is_uniquely_solvable() {
				// first number can be left out
				sudoku = sudoku_tmp;
				n_cell += 2;
				continue
			}

			sudoku_tmp.0[cell1] = sudoku.0[cell1];
			sudoku_tmp.0[cell2] = 0;
			if sudoku_tmp.is_uniquely_solvable() {
				// second number can be left out
				sudoku = sudoku_tmp;
			}

			// no number can be left out;
			n_cell += 2;
		}

		for &cell in &cell_order[50..] {
			let mut sudoku_tmp = sudoku;
			sudoku_tmp.0[cell] = 0;
			if sudoku_tmp.is_uniquely_solvable() {
				sudoku = sudoku_tmp;
			}
		}

		sudoku
	}

	/// Creates a sudoku from a byte slice.
	/// All numbers must be below 10. Empty cells are denoted by 0, clues by the numbers 1-9.
	/// The slice must be of length 81.
	pub fn from_bytes_slice(bytes: &[u8]) -> Result<Sudoku, ()> {
			if bytes.len() != 81 { return Err(()) }
			let mut sudoku = Sudoku([0; 81]);

			match bytes.iter().all(|&byte| byte <= 9) {
				true => {
					sudoku.0.copy_from_slice(bytes);
					Ok(sudoku)
				},
				false => Err(())
			}
	}

	/// Creates a sudoku from a byte array.
	/// All numbers must be below 10. Empty cells are denoted by 0, clues by the numbers 1-9.
	pub fn from_bytes(bytes: [u8; 81]) -> Result<Sudoku, ()> {
			match bytes.iter().all(|&byte| byte <= 9) {
				true => Ok(Sudoku(bytes)),
				false => Err(()),
			}
	}

	/// Reads a sudoku in the line format.
	///
	/// This is a concatenation of the digits in each cell, line by line from top to bottom.
	/// Digits must be in range of 1-9.
	/// `'_'`, `'.'` and `'0'` are accepted interchangeably as empty cells
	///
	/// An optional comment is allowed after the sudoku,
	/// separated by ASCII whitespace, commas or semicolons,
	/// that is, any of ' ', '\t', '\n', '\r', ',', ';'
	///
	/// Example:
	///
	/// ```text
	/// ..3.2.6..9..3.5..1..18.64....81.29..7.......8..67.82....26.95..8..2.3..9..5.1.3.. optional comment
	/// ```
	///
	/// Stops parsing after the first sudoku
	pub fn from_str_line(s: &str) -> Result<Sudoku, LineFormatParseError> {
		let chars = s.as_bytes();
		let mut grid = [0; N_CELLS];
		let mut i = 0;
		for (cell, &ch) in grid.iter_mut().zip(chars) {
			match ch {
				b'_' | b'.' => *cell = 0,
				b'0' ... b'9' => *cell = ch - b'0',
				// space ends sudoku before grid is filled
				b' ' | b'\t' => return Err(LineFormatParseError::NotEnoughCells(i)),
				_ => return Err(
					LineFormatParseError::InvalidEntry(
						PubEntry {
							cell: i,
							ch: s[i as usize..].chars().next().unwrap(),
						}
					)
				),
			}
			i += 1;
		}

		if i != 81 {
			return Err(LineFormatParseError::NotEnoughCells(i))
		}

		// if more than 81 elements, sudoku must be delimited
		if let Some(&ch) = chars.get(81) {
			match ch {
				// delimiters, end of sudoku
				b'\t' | b' ' | b'\r' | b'\n' | b';' | b',' => (),
				// valid cell entry => too long
				b'_' | b'.' | b'0'...b'9' => {
					return Err(LineFormatParseError::TooManyCells)
				},
				// any other char can not be part of sudoku
				// without having both length and character wrong
				// treat like comment, but with missing delimiter
				_ => return Err(LineFormatParseError::MissingCommentDelimiter),
			}
		}

		Ok(Sudoku(grid))
	}

	/// Reads a sudoku in the block format with or without field delimiters
	///
	/// Digits must be in range of 1-9.
	/// `'_'`, `'.'` and `'0'` are accepted interchangeably as empty cells
	///
	/// Optional comments are accepted after each line. They must be delimited by
	/// ' ' or '\t', i.e. a space or a tab character.
	///
	/// ```text
	/// __3_2_6__ optional comment
	/// 9__3_5__1 another comment
	/// __18_64__
	/// __81_29__
	/// 7_______8
	/// __67_82__
	/// __26_95__
	/// 8__2_3__9
	/// __5_1_3__
	/// ```
	///
	/// alternatively also with field delimiters
	///
	/// ```text
	/// __3|_2_|6__ optional comment
	/// 9__|3_5|__1 another comment
	/// __1|8_6|4__
	/// ---+---+--- comment: "-----------", i.e. '-' 11 times is also allowed
	/// __8|1_2|9__          delimiters have to be consistent across the entire
	/// 7__|___|__8          grid
	/// __6|7_8|2__
	/// ---+---+---
	/// __2|6_9|5__
	/// 8__|2_3|__9
	/// __5|_1_|3__
	/// ```
	///
	/// Stops parsing after the first sudoku
	pub fn from_str_block(s: &str) -> Result<Sudoku, BlockFormatParseError> {
		let mut grid = [0; N_CELLS];
		#[derive(PartialEq)]
		enum Format {
			Unknown,
			Delimited,
			DelimitedPlus,
			Bare,
		}
		let mut format = Format::Unknown;

		// Read a row per line
		let mut n_line_sud = 0;
		for (n_line_str, line) in s.lines().enumerate() {
			// if sudoku complete
			// enforce empty line (whitespace ignored)
			// Maybe allow comment lines in the future
			if n_line_sud == 9 {
				match line.trim().is_empty() {
					true => break,
					false => return Err(BlockFormatParseError::TooManyRows),
				}
			}

			// if delimited, check horizontal field delimiters and skip over line
			if (format == Format::Delimited || format == Format::DelimitedPlus)
			&& (n_line_str == 3 || n_line_str == 7)
			{
				if n_line_str == 3 && (line.starts_with("---+---+---") || line.starts_with("---+---+--- ")) {
					format = Format::DelimitedPlus;
				}
				if format == Format::Delimited {
					match !(line.starts_with("-----------") || line.starts_with("----------- ")) {
						true  => return Err(BlockFormatParseError::IncorrectFieldDelimiter),
						false => continue,
					}
				}
				if format == Format::DelimitedPlus {
					match !(line.starts_with("---+---+---") || line.starts_with("---+---+--- ")) {
						true  => return Err(BlockFormatParseError::IncorrectFieldDelimiter),
						false => continue,
					}
				}
			}

			let mut n_col_sud = 0;
			for (str_col, ch) in line.chars().enumerate() {
				// if line complete
				if n_col_sud == 9 {
					match ch {
						// comment separator
						' ' | '\t' => break,
						// valid entry, line too long
						'1'...'9' | '_' | '.' | '0'   => return Err(BlockFormatParseError::InvalidLineLength(n_line_sud)),
						// invalid entry, interpret as comment but enforce separation
						_ => return Err(BlockFormatParseError::MissingCommentDelimiter(n_line_sud))
					}
				}

				// if in place of vertical field delimiters
				if str_col == 3 || str_col == 7 {
					// Set parse mode on 4th char in 1st line
					if format == Format::Unknown {
						format = if ch == '|' { Format::Delimited } else { Format::Bare };
					}
					// check and skip over delimiters
					if format == Format::Delimited || format == Format::DelimitedPlus {
						match ch {
							'|'  => continue,
							_    => return Err(BlockFormatParseError::IncorrectFieldDelimiter),
						}
					}
				}

				let cell = n_line_sud * 9 + n_col_sud;
				match ch {
					'_' | '.' => grid[cell as usize] = 0,
					'0'...'9' => grid[cell as usize] = ch as u8 - b'0',
					_ => return Err(BlockFormatParseError::InvalidEntry(PubEntry{cell: cell as u8, ch })),
				}
				n_col_sud += 1;
			}
			if n_col_sud != 9 {
				return Err(BlockFormatParseError::InvalidLineLength(n_line_sud))
			}

			n_line_sud += 1;
		}
		if n_line_sud != 9 {
			return Err(BlockFormatParseError::NotEnoughRows(n_line_sud+1)) // number of rows = index of last + 1
		}
		Ok(Sudoku(grid))
	}

	/// Reads a sudoku in a variety of block formats with very few constraints.
	///
	/// '_', '.' and '0' are treated as empty cells. '1' to '9' as clues.
	/// Each line needs to have 9 valid cells.
	/// Lines that don't contain 9 valid entries are ignored.
	///
	/// Stops parsing after the first sudoku.
	///
	/// Due to the lax format rules, the only failure that can occur
	/// is that there are not enough rows.
	pub fn from_str_block_permissive(s: &str) -> Result<Sudoku, NotEnoughRows>
	{
		let mut grid = [0; N_CELLS];

		let mut valid_rows = 0;
		for line in s.lines() {
			let mut row_vals = [0; 9];
			let mut nums_in_row = 0;
			for ch in line.chars() {
				if ['.', '_'].contains(&ch) {
					row_vals[nums_in_row] = 0;
					nums_in_row += 1;
				} else if '0' <= ch && ch <= '9' {
					row_vals[nums_in_row] = ch as u8 - b'0';
					nums_in_row += 1;
				}
				// full sudoko row, write to grid
				// ignore anything after in same row
				if nums_in_row == 9 {
					grid[valid_rows*9..valid_rows*9 + 9].copy_from_slice(&row_vals);
					valid_rows += 1;
					break
				}
			}
			if valid_rows == 9 {
				return Ok(Sudoku(grid))
			}
		}
		Err(NotEnoughRows(valid_rows as u8))
	}

	/// Find a solution to the sudoku. If multiple solutions exist, it will not find them and just stop at the first.
	/// Return `None` if no solution exists.
    pub fn solve_one(self) -> Option<Sudoku> {
		let mut buf = [[0; 81]];
		match self.solve_at_most_buffer(&mut buf, 1) == 1 {
			true => Some(Sudoku(buf[0])),
			false => None,
		}
    }

    /// Solve sudoku and return solution if solution is unique.
	pub fn solve_unique(self) -> Option<Sudoku> {
		// without at least 8 digits present, sudoku has multiple solutions
		// bitmask
		let mut nums_contained: u16 = 0;
		// same with less than 17 clues
		let mut n_clues = 0;
		self.iter()
			.filter_map(|id| id)
			.for_each(|num| {
				nums_contained |= 1 << num;
				n_clues += 1;
			});
		if n_clues < 17 || nums_contained.count_ones() < 8 {
			return None
		};

		let mut solution = [[0; 81]];
		let n_solutions = self.solve_at_most_buffer(&mut solution, 2);
		match n_solutions == 1 {
			true => Some(Sudoku(solution[0])),
			false => None,
		}
	}

	/// Counts number of solutions to sudoku up to `limit`
	/// This solves the sudoku but does not return the solutions which allows for slightly faster execution.
	pub fn count_at_most(self, limit: usize) -> usize {
		SudokuSolver::from_sudoku(self)
			.ok()
			.map_or(0, |solver| solver.count_at_most(limit))
	}

	/// Checks whether sudoku has one and only one solution.
	/// This solves the sudoku but does not return the solution which allows for slightly faster execution.
	pub fn is_uniquely_solvable(self) -> bool {
		self.count_at_most(2) == 1
	}

	/// Solve sudoku and return the first `limit` solutions it finds. If less solutions exist, return only those. Return `None` if no solution exists.
	/// No specific ordering of solutions is promised. It can change across versions.
    pub fn solve_at_most(self, limit: usize) -> Vec<Sudoku> {
		SudokuSolver::from_sudoku(self)
			.ok()
			.map_or(vec![], |solver| solver.solve_at_most(limit))
	}

	/// Counts number of solutions to sudoku up to `limit` and writes any solution found into `target`
	/// up to its capacity. Additional solutions will be counted but not saved.
	/// No specific ordering of solutions is promised. It can change across versions.
	/// This is primarily meant for C FFI.
    pub fn solve_at_most_buffer(self, target: &mut [[u8; 81]], limit: usize) -> usize {
		SudokuSolver::from_sudoku(self)
			.ok()
			.map_or(0, |solver| solver.solve_at_most_buffer(target, limit))
	}

	/// Check whether the sudoku is solved.
	pub fn is_solved(&self) -> bool {
		SudokuSolver::from_sudoku(*self)
			.ok()
			.as_ref()
			.map_or(false, SudokuSolver::is_solved)
	}

	/// Returns number of filled cells
	pub fn n_clues(&self) -> u8 {
		self.0.iter().filter(|&&num| num != 0).count() as u8
	}

	/// Perform various transformations that create a different but equivalent sudoku.
	/// The transformations preserve the sudoku's validity and the amount of solutions
	/// as well a the applicability of solution strategies.
	/// Shuffling can be used to quickly generate sudokus of the same difficulty as a given sudoku.
	///
	/// Transformations that are applied:
	/// - Relabel numbers, e.g. swap all 1s and all 3s (9! permutations)
	/// - Permute rows within their band and columns within their stack (3!<super>3 * 2</super> permutations)
	/// - Permute stacks and bands (3!<super>2</super> permutations)
	/// - Transpose the board, i.e. mirror it along the diagonal (2 permutations)
	///   The remaining rotations as well as mirrorings can be produced by a combination with the other transformations
	///
	/// This results in a total of up to 2 * 9! * 3!<super>8</super> = 1,218,998,108,160 permutations
	/// Less permutations exists if the sudoku is symmetrical in respect to some combinations of the transformations
	/// The vast majority of sudokus do not have any such symmetries (automorphisms). The highest number of automorphisms
	/// a sudoku can have is 648 and ~99.99% of all non-equivalent sudokus have only 1, the identity transformation.

	// TODO: Deduplicate the shuffle_*lines_or_chutes* functions
	//		 for some reason the shuffle_bands and shuffle_stacks functions work faster in their current form
	// 		 rather than with a generic function abstracting over both.
	pub fn shuffle(&mut self) {
		// SmallRng is a good 10% faster, but it uses XorShiftRng which can fail some statistical tests
		// There are some adaptions that fix this, but I don't know if Rust implements them.
		//let rng = &mut ::rand::rngs::SmallRng::from_rng(::rand::thread_rng()).unwrap();
		let rng = &mut ::rand::thread_rng();

		self.shuffle_digits(rng);
		self.shuffle_bands(rng);
		self.shuffle_stacks(rng);
		for i in 0..3 {
			self.shuffle_cols_of_stack(rng, i);
			self.shuffle_rows_of_band(rng, i);
		}
		if rng.gen() {
			self.transpose();
		}
	}

	#[inline]
	fn shuffle_digits(&mut self, rng: &mut R) {
		// 0 (empty cell) always maps to 0
		let mut digits = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

		// manual top-down Fisher-Yates shuffle. Needs only 1 ranged random num rather than 9
		let mut permutation = rng.gen_range(0, 362880u32); // 9!
		for n_choices in (1..10).rev() {
			let num = permutation % n_choices;
			permutation /= n_choices;
			digits.swap(n_choices as usize, 1 + num as usize);
		}

		for num in self.0.iter_mut() {
			*num = digits[*num as usize];
		}
	}

	#[inline]
	fn shuffle_rows_of_band(&mut self, rng: &mut R, band: u8) {
		debug_assert!(band < 3);
		let first_row = band*3;

		// Fisher-Yates-Shuffle
		self.swap_rows(first_row, rng.gen_range(first_row, first_row+3));
		self.swap_rows(first_row+1, rng.gen_range(first_row+1, first_row+3));
	}

	#[inline]
	fn shuffle_cols_of_stack(&mut self, rng: &mut R, stack: u8) {
		debug_assert!(stack < 3);
		let first_col = stack*3;

		// Fisher-Yates-Shuffle
		self.swap_cols(first_col, rng.gen_range(first_col, first_col+3));
		self.swap_cols(first_col+1, rng.gen_range(first_col+1, first_col+3));
	}

	#[inline]
	fn shuffle_bands(&mut self, rng: &mut R) {
		// Fisher-Yates-Shuffle
		self.swap_bands(0, rng.gen_range(0, 3));
		self.swap_bands(1, rng.gen_range(1, 3));
	}

	#[inline]
	fn shuffle_stacks(&mut self, rng: &mut R) {
		// Fisher-Yates-Shuffle
		self.swap_stacks(0, rng.gen_range(0, 3));
		self.swap_stacks(1, rng.gen_range(1, 3));
	}

	#[inline]
	fn swap_rows(&mut self, row1: u8, row2: u8) {
		if row1 == row2 { return }
		let start1 = (row1*9) as usize;
		let start2 = (row2*9) as usize;
		self.swap_cells(
			(start1..start1+9).zip(start2..start2+9)
		)
	}

	#[inline]
	fn swap_cols(&mut self, col1: u8, col2: u8) {
		if col1 == col2 { return }
		debug_assert!(col1 < 9);
		debug_assert!(col2 < 9);
		self.swap_cells(
			(0..9).map(|row| (row*9 + col1 as usize, row*9 + col2 as usize))
		)
	}

	#[inline]
	fn swap_stacks(&mut self, stack1: u8, stack2: u8) {
		if stack1 == stack2 { return }
		debug_assert!(stack1 < 3);
		debug_assert!(stack2 < 3);
		for inner_col in 0..3 {
			self.swap_cols(stack1*3+inner_col, stack2*3+inner_col);
		}
	}

	#[inline]
	fn swap_bands(&mut self, band1: u8, band2: u8) {
		if band1 == band2 { return }
		debug_assert!(band1 < 3);
		debug_assert!(band2 < 3);
		for inner_row in 0..3 {
			self.swap_cols(band1*3+inner_row, band2*3+inner_row);
		}
	}

	#[inline]
	fn transpose(&mut self) {
		use ::std::iter::repeat;
		self.swap_cells(
			(0..9)
				.flat_map(|row| repeat(row).zip(row+1..9))
				.map(|(row, col)| (row*9+col, col*9+row))
		)
	}

	// takes iter of cell index pairs and swaps the corresponding cells
	#[inline]
	fn swap_cells(&mut self, iter: impl Iterator<Item=(usize, usize)>) {
		for (idx1, idx2) in iter {
			debug_assert!(idx1 != idx2);

			let a = self.0[idx1];
			let b = self.0[idx2];
			self.0[idx1] = b;
			self.0[idx2] = a;
		}
	}

    /// Returns an Iterator over sudoku, going from left to right, top to bottom
    pub fn iter(&self) -> Iter {
        self.0.iter().map(num_to_opt)
    }

	/// Returns a byte array for the sudoku.
	/// Empty cells are denoted by 0, clues by the numbers 1-9.
	pub fn to_bytes(self) -> [u8; 81] {
		self.0
	}

	/// Returns a representation of the sudoku in line format that can be printed
	/// and which derefs into a &str
	///
	/// ```
	/// use sudoku::Sudoku;
	///
	/// let mut grid = [0; 81];
	/// grid[3] = 5;
	/// let sudoku = Sudoku::from_bytes(grid).unwrap();
	/// let line = sudoku.to_str_line(); // :SudokuLine
	/// println!("{}", line);
	///
	/// let line_str: &str = &line;
	/// assert_eq!(
	///		"...5.............................................................................",
	///     line_str
	///	);
	/// ```
	pub fn to_str_line(&self) -> SudokuLine {
		let mut chars = [0; 81];
		for (char_, entry) in chars.iter_mut().zip(self.iter()) {
			*char_ = match entry {
				Some(num) => num + b'0',
				None => b'.',
			};
		}
		SudokuLine(chars)
	}

	/// Returns a value that, prints a block representation of the sudoku
	/// when formatted via the `Display` trait.
	///
	///
	/// ```
	/// use sudoku::Sudoku;
	///
	/// let mut grid = [0; 81];
	/// grid[3] = 5;
    /// grid[36..45].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9]);
	/// let sudoku = Sudoku::from_bytes(grid).unwrap();
	/// let block = sudoku.display_block(); // :SudokuBlock
	///
	/// let block_string = format!("{}", block);
	/// assert_eq!(
	///     &block_string,
	/// "
	/// ___ 5__ ___
	/// ___ ___ ___
	/// ___ ___ ___
	///
	/// ___ ___ ___
	/// 123 456 789
	/// ___ ___ ___
	///
	/// ___ ___ ___
	/// ___ ___ ___
	/// ___ ___ ___"
	///	);
	/// ```
	pub fn display_block(&self) -> SudokuBlock {
		SudokuBlock(self.0)
	}
}

fn num_to_opt(num: &u8) -> Option<u8> {
	if *num == 0 { None } else { Some(*num) }
}

impl fmt::Display for Sudoku {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.to_str_line())
	}
}


/// Container for the &str representation of a sudoku
// MUST ALWAYS contain valid utf8
#[derive(Copy, Clone)]
pub struct SudokuLine([u8; 81]);

/// The ordering is lexicographical in the cells of the sudoku
/// going from left to right, top to bottom
impl PartialOrd for SudokuLine {
	fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
		// the &str representation uses '.', which is below '0' and therefore
		// this orders just like the regular sudoku would.
		// deref into &str and cmp
		(**self).partial_cmp(other)
	}
}

impl Ord for SudokuLine {
	fn cmp(&self, other: &Self) -> cmp::Ordering {
		// deref into &str and cmp
		(**self).cmp(other)
	}
}

impl hash::Hash for SudokuLine {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher
	{
		(**self).hash(state)
	}
}

impl PartialEq for SudokuLine {
	fn eq(&self, other: &SudokuLine) -> bool {
		self.0[..] == other.0[..]
	}
}

impl Eq for SudokuLine {}

impl fmt::Debug for SudokuLine {
	fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
		self.0.fmt(fmt)
	}
}

impl ops::Deref for SudokuLine {
	type Target = str;
	fn deref(&self) -> &Self::Target {
		str::from_utf8(&self.0).unwrap()
	}
}

impl fmt::Display for SudokuLine {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.deref())
	}
}

/// Sudoku that will be printed in block format.
/// This exists primarily for debugging.
#[derive(Copy, Clone)]
pub struct SudokuBlock([u8; 81]);

/// The ordering is lexicographical in the cells of the sudoku
/// going from left to right, top to bottom
impl PartialOrd for SudokuBlock {
	fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
		self.0[..].partial_cmp(&other.0[..])
	}
}

impl Ord for SudokuBlock {
	fn cmp(&self, other: &Self) -> cmp::Ordering {
		self.0[..].cmp(&other.0[..])
	}
}

impl hash::Hash for SudokuBlock {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher
	{
		self.0[..].hash(state)
	}
}

impl PartialEq for SudokuBlock {
	fn eq(&self, other: &SudokuBlock) -> bool {
		self.0[..] == other.0[..]
	}
}

impl Eq for SudokuBlock {}

impl fmt::Debug for SudokuBlock {
	fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
		self.0.fmt(fmt)
	}
}

impl fmt::Display for SudokuBlock {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		for entry in self.0.iter().enumerate().map(|(cell, &num)| Entry { cell: cell as u8, num } ) {
			match (entry.row(), entry.col()) {
				(_, 3) | (_, 6) => write!(f, " ")?,    // seperate fields in columns
				(3, 0) | (6, 0) => write!(f, "\n\n")?, // separate fields in rows
				(_, 0)          => write!(f, "\n")?,   // separate lines not between fields
				_ => {},
			};
            match entry.num() {
                0 => write!(f, "_")?,
                1...9 => write!(f, "{}", entry.num())?,
                _ => unreachable!(),
            };
		}
		Ok(())
	}
}
