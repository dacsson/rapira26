//! Disassembler of Lama VM bytecode file

use crate::bytecode::Instruction;
use crate::decoder::{Decoder, DecoderError};
use std::collections::HashMap;
use std::ffi::CString;
use std::fmt::Display;
use std::io::{BufRead, BufReader, Cursor, Read};

// Memory layout of the bytecode file
// +------------------------------------+
// |           File Header              |
// |------------------------------------|
// |  int32: S       | 4 bytes          |
// |  int32: glob_count | 4 bytes       |
// |  int32: P       | 4 bytes          |
// |  P × (int32, int32) | 8 bytes each |
// +------------------------------------+
// |           String Table             |
// |------------------------------------|
// |  S bytes        | Variable         |
// |  e.g., "string1\0string2\0"        |
// +------------------------------------+
// |           Code Region              |
// |------------------------------------|
// |  Variable bytes | Instructions     |
// |  e.g., 0x01 0x02 ... 0xFF          |
// +------------------------------------+
#[derive(Clone)]
pub struct Bytefile {
    pub stringtab_size: u32,
    pub global_area_size: u32,
    pub public_symbols_number: u32,
    pub public_symbols: Vec<(u32, u32)>,
    pub string_table: Vec<u8>,
    pub code_section: Vec<u8>,      // Kept raw for later interpretation
    pub main_offset: u32,           // "main" function offset, a.k.a entry point
    labels: HashMap<String, usize>, // Mapping from label names to their offsets in the code section
}

#[derive(Debug)]
pub enum BytefileError {
    InvalidFileFormat,
    FileReadFailed,
    MemoryAllocationFailed,
    UnexpectedEOF,
    NoCodeSection,
    InvalidStringIndexInStringTable,
    MainNotFound,
    InvalidPublicSymbolOffset(u32, u32),
}

impl Bytefile {
    /// Creates a new Bytefile
    ///
    /// It is not yet saved nor validated
    pub fn new() -> Self {
        Self {
            stringtab_size: 0,
            global_area_size: 0,
            public_symbols_number: 0,
            public_symbols: Vec::new(),
            string_table: Vec::new(),
            code_section: Vec::new(),
            main_offset: 0,
            labels: HashMap::new(),
        }
    }

    /// Adds a string to the string section of bytefile
    pub fn add_string(&mut self, string: String) -> u32 {
        let offset = self.stringtab_size;
        self.string_table.extend(string.as_bytes());
        // Push null terminator
        self.string_table.push(0);
        self.stringtab_size += string.as_bytes().len() as u32 + 1;
        offset
    }

    /// Add raw code to the code section of bytefile
    pub fn add_code(&mut self, code: Vec<u8>) {
        self.code_section.extend(code);
    }

    /// Encode all instructions and add them to the code section of bytefile
    pub fn add_instructions(&mut self, instructions: &[Instruction]) -> Result<(), DecoderError> {
        // TODO: refactor for less O(n) complexity

        let mut instructions = instructions.to_vec();

        // Traverse labels and resolve offsets, using
        // mock data
        let mut offset = self.code_section.len();
        for instruction in &instructions {
            match instruction {
                Instruction::LABEL { name } => {
                    let offset = i32::try_from(offset)
                        .map_err(|_| DecoderError::ReadingMoreThenCodeSection)?;

                    if self.labels.insert(name.clone(), offset as usize).is_some() {
                        // FIXME: duplicate
                        return Err(DecoderError::UnknownLabel(name.clone()));
                    }
                }
                Instruction::JMP { .. } | Instruction::CJMP { .. } => {
                    offset += 5;
                }
                Instruction::CALL { .. } => {
                    offset += 9; // opcode + target i32 + argument count i32
                }
                instruction => {
                    offset += Decoder::encode(instruction)?.len();
                }
            }
        }

        // Now put resolved offsets into the jump instructions
        for instruction in &mut instructions {
            if let Instruction::JMP { dest }
            | Instruction::CJMP { dest, .. }
            | Instruction::CALL { dest, .. } = instruction
            {
                let name = &dest.name;
                if let Some(offset) = self.labels.get(name) {
                    dest.offset = Some(*offset as i32);
                }
            }
        }

        // Now that labels are resolved, add instructions to the code section
        for instruction in instructions {
            self.add_instruction(&instruction)?;
        }
        Ok(())
    }

    /// Adds an instruction to the code section of bytefile, encoding it into byte buffer first
    // TODO: make private
    pub fn add_instruction(&mut self, instruction: &Instruction) -> Result<(), DecoderError> {
        let encoded = Decoder::encode(instruction)?;
        self.code_section.extend(encoded);
        Ok(())
    }

    /// Addds a public symbol to the public symbols section of bytefile
    ///
    /// The name for it should already be in the string table
    pub fn add_public_symbol(&mut self, name: &str, offset: u32) -> Result<(), BytefileError> {
        // First find the index of string (offset)
        let string_offset = self.find_string_offset(name);

        if string_offset.is_none() {
            return Err(BytefileError::InvalidStringIndexInStringTable);
        }

        self.public_symbols.push((string_offset.unwrap(), offset));
        self.public_symbols_number += 1;

        Ok(())
    }

    /// Returns offset of the given string in the string table, if found
    pub fn find_string_offset(&self, string: &str) -> Option<u32> {
        let mut offset = 0;

        while offset < self.string_table.len() {
            let remaining = &self.string_table[offset..];
            let terminator = remaining.iter().position(|&byte| byte == 0)?;

            if &remaining[..terminator] == string.as_bytes() {
                return Some(offset as u32);
            }

            offset += terminator + 1;
        }

        None
    }

    /// Returns the current offset relative to the start of the whole bytefile
    pub fn get_current_global_offset(&self) -> usize {
        // Metadata size
        let stringtab_size_size = std::mem::size_of::<u32>();
        let global_area_size_size = std::mem::size_of::<u32>();
        let public_symbols_number_size = std::mem::size_of::<u32>();
        let public_symbols_size = self.public_symbols.len() * std::mem::size_of::<(u32, u32)>();

        // String table size
        let string_table_size = self.string_table.len();

        // Current code section size
        let code_section_size = self.code_section.len();

        return stringtab_size_size
            + global_area_size_size
            + public_symbols_number_size
            + public_symbols_size
            + string_table_size
            + code_section_size;
    }

    /// Returns the current offset relative to the start of the code section
    ///
    /// For overall offset calculation, use [`Bytefile::get_current_global_offset`]
    pub fn get_current_offset(&self) -> usize {
        self.code_section.len()
    }

    /// Encodes the bytefile into a binary format
    ///
    /// The `*.rbc` files are exactly that (this output)
    pub fn encode(&self) -> Vec<u8> {
        let mut output = vec![];

        // Push stringtab size
        output.extend(self.stringtab_size.to_le_bytes());

        // Push global area size
        output.extend(self.global_area_size.to_le_bytes());

        // Push public symbols number
        output.extend(self.public_symbols_number.to_le_bytes());

        // Push public symbols
        // P × (int32, int32) | 8 bytes each
        for (offset, name_offset) in &self.public_symbols {
            output.extend(offset.to_le_bytes());
            output.extend(name_offset.to_le_bytes());
        }

        // Push string table
        output.extend(&self.string_table);

        // Push code section
        output.extend(&self.code_section);

        output
    }

    /// Parse a bytecode file into a Bytefile struct.
    /// Leaves code section raw (as raw bytes) to be interpreted later by a [`Decoder`],
    /// while all other sections are parsed and stored to be easily accessed.
    pub fn parse(source: Vec<u8>) -> Result<Bytefile, BytefileError> {
        let source_len = source.len();
        let mut reader = BufReader::new(Cursor::new(source));

        let mut buf = [0u8; 4];
        reader
            .read_exact(&mut buf)
            .map_err(|_| BytefileError::UnexpectedEOF)?;
        let stringtab_size = u32::from_le_bytes(buf);

        buf.fill(0);
        reader
            .read_exact(&mut buf)
            .map_err(|_| BytefileError::UnexpectedEOF)?;
        let global_area_size = u32::from_le_bytes(buf);

        buf.fill(0);
        reader
            .read_exact(&mut buf)
            .map_err(|_| BytefileError::UnexpectedEOF)?;
        let public_symbols_number = u32::from_le_bytes(buf);

        // Read public symbol table
        // P × (int32, int32) | 8 bytes each
        let mut public_symbols = Vec::with_capacity(public_symbols_number as usize);
        for _ in 0..public_symbols_number {
            buf.fill(0);
            reader
                .read_exact(&mut buf)
                .map_err(|_| BytefileError::UnexpectedEOF)?;
            let symbol = u32::from_le_bytes(buf);
            reader
                .read_exact(&mut buf)
                .map_err(|_| BytefileError::UnexpectedEOF)?;
            let name = u32::from_le_bytes(buf);
            public_symbols.push((symbol, name));
        }

        // Read string table
        // let mut byte = [0u8; 1];
        let mut string_table = vec![0u8; stringtab_size as usize];
        reader
            .read_exact(&mut string_table)
            .map_err(|_| BytefileError::UnexpectedEOF)?;

        // Find "main" entry point in public symbols
        let main_offset = public_symbols
            .iter()
            .find(|(s_index, _)| {
                let slice: &[u8] = &string_table[*s_index as usize..];
                let first_null = slice
                    .iter()
                    .position(|&b| b == 0)
                    .ok_or(BytefileError::InvalidStringIndexInStringTable);

                if let Ok(first_null) = first_null {
                    let buff = slice[..=first_null].to_vec();

                    if buff == b"main\0".to_vec() {
                        return true;
                    } else {
                        return false;
                    }
                }
                false
            })
            .map(|(_, offset)| *offset)
            .ok_or(BytefileError::MainNotFound)?;

        // Read code section
        let bytes_till_end = source_len - reader.buffer().len();
        let mut code_section = Vec::with_capacity(bytes_till_end as usize);
        reader
            .read_to_end(&mut code_section)
            .map_err(|_| BytefileError::UnexpectedEOF)?;

        // Check public symbols offsets are within bounds
        for (s_index, offset) in &public_symbols {
            if *offset >= code_section.len() as u32 {
                return Err(BytefileError::InvalidPublicSymbolOffset(
                    *offset,
                    code_section.len() as u32,
                ));
            }

            if *s_index >= string_table.len() as u32 {
                return Err(BytefileError::InvalidStringIndexInStringTable);
            }
        }

        Ok(Bytefile {
            stringtab_size,
            global_area_size,
            public_symbols_number,
            public_symbols,
            string_table,
            code_section,
            main_offset,
            labels: HashMap::new(),
        })
    }

    /// Given a strings as array of bytes (including null terminators), find nth string
    ///
    /// Do not use this in interpreter, use [`Bytefile::get_string_at_offset`] instead
    pub fn get_string_at(&self, index: usize) -> Result<Vec<u8>, BytefileError> {
        let mut reader = BufReader::new(Cursor::new(&self.string_table));
        let mut strings = Vec::new();

        for _ in 0..self.stringtab_size {
            let mut buff = vec![];
            reader
                .read_until(0x00, &mut buff)
                .map_err(|_| BytefileError::InvalidStringIndexInStringTable)?;
            strings.push(buff);
        }

        #[cfg(feature = "runtime_checks")]
        if index >= strings.len() {
            return Err(BytefileError::InvalidStringIndexInStringTable);
        }

        Ok(strings[index].to_vec())
    }

    /// Given a strings as array of bytes (including null terminators), read string to null-terminator at offset `offset`
    pub fn get_string_at_offset(&self, offset: usize) -> Result<&[u8], BytefileError> {
        #[cfg(feature = "runtime_checks")]
        if offset >= self.string_table.len() {
            return Err(BytefileError::InvalidStringIndexInStringTable);
        }

        let slice = &self.string_table[offset..];
        let first_null = slice
            .iter()
            .position(|&b| b == 0)
            .ok_or(BytefileError::InvalidStringIndexInStringTable)?;
        let buff = &slice[..=first_null];

        Ok(buff)
    }

    /// Create a dummy Bytefile for testing purposes
    pub fn new_dummy() -> Self {
        Bytefile {
            stringtab_size: 0,
            global_area_size: 0,
            public_symbols_number: 0,
            code_section: vec![0; 100],
            string_table: vec![],
            public_symbols: vec![],
            main_offset: 0,
            labels: HashMap::new(),
        }
    }

    /// Push an arbitrary string in string table for testing purposes
    pub fn put_string(&mut self, str: CString) {
        let slice = str.as_bytes_with_nul();
        slice.iter().for_each(|b| self.string_table.push(*b));
        self.stringtab_size += 1;
    }
}

impl Display for Bytefile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // FIXME: this is illigal and punishible by death
        let mut decoder = Decoder::new(self.clone());

        write!(f, "\\ --------- Bytefile Dump ----------\n")?;
        write!(f, "\\ - String Table Size: {}\n", self.stringtab_size)?;
        write!(f, "\\ - Global Area Size: {}\n", self.global_area_size)?;
        write!(
            f,
            "\\ - Public Symbol Table Size: {}\n",
            self.public_symbols_number
        )?;
        write!(
            f,
            "\\ - Code Section Byte Size: {}\n",
            self.code_section.len()
        )?;

        write!(f, "\\ - Public symbols: \n")?;
        for (s, n) in &self.public_symbols {
            write!(f, "\\  - {}: {}\n", s, n)?;
        }

        let str_table = String::from_utf8(self.string_table.clone()).unwrap();
        write!(f, "\\ - String table raw: {:?}\n", self.string_table)?;
        write!(f, "\\ - String Table: {}\n", str_table)?;

        write!(f, "\\-----------------------------\n")?;

        // TODO: mark each function
        while decoder.ip < self.code_section.len() {
            let encoding = decoder.next::<u8>().map_err(|_| std::fmt::Error)?;
            let instr = decoder.decode(encoding).map_err(|_| std::fmt::Error)?;
            write!(f, "\n{}", instr)?;
        }

        write!(f, "")
    }
}

impl Display for BytefileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BytefileError::InvalidFileFormat => write!(f, "Invalid file format"),
            BytefileError::FileReadFailed => write!(f, "File read failed"),
            BytefileError::MemoryAllocationFailed => write!(f, "Memory allocation failed"),
            BytefileError::UnexpectedEOF => write!(f, "Unexpected end of file"),
            BytefileError::NoCodeSection => write!(f, "No code section"),
            BytefileError::InvalidStringIndexInStringTable => {
                write!(f, "Invalid string index in string table")
            }
            BytefileError::MainNotFound => write!(f, "Main function not found"),
            BytefileError::InvalidPublicSymbolOffset(offset, max) => {
                write!(f, "Invalid public symbol offset: {} (max: {})", offset, max)
            }
        }
    }
}

impl std::error::Error for BytefileError {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_minimal_file() -> Result<(), Box<dyn std::error::Error>> {
        // ~ =>  xxd dump/test1.bc
        // 00000000: 0500 0000 0100 0000 0100 0000 0000 0000  ................
        // 00000010: 0000 0000 6d61 696e 0052 0200 0000 0000  ....main.R......
        // 00000020: 0000 1002 0000 0010 0300 0000 015a 0100  .............Z..
        // 00000030: 0000 4000 0000 0018 5a02 0000 005a 0400  ..@.....Z....Z..
        // 00000040: 0000 2000 0000 0071 16ff                 .. ....q..
        let data: Vec<u8> = vec![
            0x05, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x6d, 0x61, 0x69, 0x6e, 0x00, 0x52, 0x02, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x02, 0x00, 0x00, 0x00, 0x10, 0x03, 0x00,
            0x00, 0x00, 0x01, 0x5a, 0x01, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x18,
            0x5a, 0x02, 0x00, 0x00, 0x00, 0x5a, 0x04, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00,
            0x00, 0x71, 0x16,
        ];

        let bytefile: Bytefile = Bytefile::parse(data)?;

        assert_eq!(bytefile.stringtab_size, 5);
        assert_eq!(bytefile.global_area_size, 1);
        assert_eq!(bytefile.public_symbols_number, 1);

        // Find "main" function name stored in string table
        let main_str = bytefile.get_string_at(0)?;
        assert_eq!(String::from_utf8(main_str)?, "main\0");

        Ok(())
    }

    #[test]
    fn empty_string_is_not_found_at_end_of_table() {
        let mut bytefile = Bytefile::new();
        bytefile.add_string("main".to_string());

        assert_eq!(bytefile.find_string_offset(""), None);

        let empty_offset = bytefile.add_string(String::new());
        bytefile.add_string("following".to_string());

        assert_eq!(bytefile.find_string_offset(""), Some(empty_offset));
        assert_eq!(
            bytefile
                .get_string_at_offset(empty_offset as usize)
                .unwrap(),
            b"\0"
        );
    }
}
