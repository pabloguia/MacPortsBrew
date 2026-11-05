// SPDX-License-Identifier: Apache-2.0

//! A pure, in-process Mach-O reader (R-27, work order 16 part H).
//!
//! Nothing here opens a file, walks a package or knows a provider. It takes
//! bytes a caller read and answers what they say, in stages, so a 200 MB file
//! is read by windows and never whole:
//!
//! 1. [`classify`]: the first [`HEAD_BYTES`] of a file (or all of it, if it is
//!    shorter) and its length, to a fat table's byte ranges, a thin range, a
//!    shebang, or a kind that carries no load commands.
//! 2. [`plan_slice`]: the first [`SLICE_HEADER_BYTES`] of one range (or all of
//!    it, if it is shorter), to how many bytes the commands need.
//! 3. [`read_slice`]: that window, to the slice's references.
//!
//! **An error is never "not Mach-O".** A file this reader cannot answer for is
//! a [`MachoError`], which the caller turns into `Unchecked`: a fat magic whose
//! table does not fit must not hide as a Java class, and a truncated command
//! list must not read as a clean one (R-27 findings 16 and 27). Every slice is
//! answered, because a reference in the slice this host would not run is still
//! a reference (R-27 finding 17).
//!
//! The answers are closed identifiers and raw bytes. No text from a file enters
//! an error, and a name stays bytes: it is compared, never decoded lossily.

// A panic mid-migration strands a run, so this module cannot index, overflow or
// unwrap. Its tests live in `crates/adapters/tests/macho.rs`.
#![deny(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::expect_used
)]

/// How much of a file [`classify`] needs: this many bytes, or the whole file if
/// it is shorter. A larger fat table than [`MAX_SLICES`] allows is refused
/// without reading it, so this always holds the table.
pub const HEAD_BYTES: usize = 4096;

/// How much of a slice [`plan_slice`] needs: this many bytes, or the whole
/// slice if it is shorter. The 64-bit Mach-O header's size.
pub const SLICE_HEADER_BYTES: usize = 32;

/// The most slices a fat file may declare. 3 seen (R-27 E-2).
pub const MAX_SLICES: u32 = 16;

/// The most load commands a slice may declare. 70 seen (R-27 E-7).
pub const MAX_NCMDS: u32 = 8192;

/// The most bytes of load commands a slice may declare. 5.3 KB seen (R-27 E-7).
pub const MAX_SIZEOFCMDS: u32 = 1_048_576;

/// The most bytes a caller reads for one file by following these stages: one
/// head, then one slice header and one window per slice. Not checked at run
/// time because it follows from the caps above, and a test holds the sum.
pub const MAX_BYTES_PER_FILE: u64 = 16_782_336;

/// A byte range of a file, inside the file's length by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SliceRange {
    /// Where the slice starts.
    pub offset: u64,
    /// How long it is. Never zero.
    pub size: u64,
}

/// A kind of file that carries no dynamic load commands to read (R-27 finding 7).
/// The caller reads nothing more of it and states no count; a package holding only
/// these has no load commands, which is what the basis says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkippedKind {
    /// An `ar` archive.
    Archive,
    /// `MH_OBJECT`.
    Object,
}

/// What a script's first line names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shebang {
    /// An absolute interpreter, as bytes.
    Absolute(Vec<u8>),
    /// An absolute interpreter whose last component is `env`, as bytes. What runs
    /// is `PATH`'s decision at run time and cannot be read from the file (R-27
    /// finding 12), yet the path is still an answer: an `env` under another
    /// provider's prefix is a reference to it. So the caller tests the path as
    /// it tests [`Shebang::Absolute`], and only then treats the rest as
    /// indeterminate.
    Env(Vec<u8>),
    /// Anything else, as bytes, possibly empty: `#! .` exists (R-27 E-5).
    Relative(Vec<u8>),
}

/// What a file's first bytes say it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Classified {
    /// No magic this reader knows, so no load commands and no shebang.
    NotObject,
    /// Carries no load commands.
    Skipped(SkippedKind),
    /// A `#!` script.
    Script(Shebang),
    /// A fat file: every slice, in table order. At least one.
    Fat(Vec<SliceRange>),
    /// A thin Mach-O: one range, the whole file.
    Thin(SliceRange),
}

/// What one slice's header says to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlicePlan {
    /// Carries no load commands: an `ar` archive or `MH_OBJECT` in a fat
    /// wrapper, or a thin `MH_OBJECT`.
    Skipped(SkippedKind),
    /// Read this many bytes from the slice's start and pass them to
    /// [`read_slice`]. At most [`MAX_SIZEOFCMDS`] plus a header.
    Window {
        /// The window's length, inside the slice.
        len: u64,
    },
}

/// The file types read. R-27 finding 7 names exactly these three; `MH_DYLINKER`
/// is not among them and the sweep saw none, so it is an unknown type here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExaminedType {
    /// `MH_EXECUTE`.
    Execute,
    /// `MH_DYLIB`.
    Dylib,
    /// `MH_BUNDLE`.
    Bundle,
}

/// Which command a [`LoadRef`] came from. `LC_ID_DYLIB` is none of these: a
/// file's own name is excluded by kind, wherever it sits (R-27 finding 18).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadKind {
    /// `LC_LOAD_DYLIB`.
    Load,
    /// `LC_LOAD_WEAK_DYLIB`.
    Weak,
    /// `LC_REEXPORT_DYLIB`.
    Reexport,
    /// `LC_LAZY_LOAD_DYLIB`.
    Lazy,
    /// `LC_LOAD_UPWARD_DYLIB`.
    Upward,
    /// `LC_LOAD_DYLINKER`.
    Dylinker,
    /// `LC_DYLD_ENVIRONMENT`. Its name is a `NAME=value` string, not a path.
    DyldEnvironment,
}

/// One reference a slice's load commands carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadRef {
    /// The command it came from.
    pub kind: LoadKind,
    /// The bytes up to the first NUL inside the command. Never empty.
    pub name: Vec<u8>,
}

/// What one slice's load commands say, in command order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceRefs {
    /// The header's `cputype`, as written.
    pub cputype: u32,
    /// The header's file type. An `@executable_path` rpath outside an
    /// executable depends on the loader, so the caller needs this (R-27
    /// finding 4).
    pub file_type: ExaminedType,
    /// Every dylib, dynamic-linker and dyld-environment command.
    pub loads: Vec<LoadRef>,
    /// Every `LC_RPATH` entry.
    pub rpaths: Vec<Vec<u8>>,
}

/// Which structure did not hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MalformedKind {
    /// The bytes given do not match the length given: more than the file, or
    /// fewer than the stage asks for.
    HeadInconsistent,
    /// A fat table that does not fit the file.
    FatTable,
    /// A fat table with no slices.
    FatEmpty,
    /// `offset + size` overflows or passes the end of the file.
    SliceBeyondFile,
    /// A slice that starts inside the fat table.
    SliceOverlapsTable,
    /// A slice of size zero.
    SliceEmpty,
    /// A range too short for a Mach-O header.
    SliceHeaderShort,
    /// A range whose magic is neither Mach-O nor `ar`, a nested fat included.
    SliceNotMachO,
    /// The header plus `sizeofcmds` passes the end of the slice.
    CommandsBeyondSlice,
    /// The window is shorter than the header it holds declares.
    WindowShort,
    /// A window whose file type this stage does not read.
    NotExaminedType,
    /// A command's header or `cmdsize` passes the end of the commands.
    CommandBeyondCommands,
    /// `cmdsize` under 8.
    CommandTooSmall,
    /// `cmdsize` too small for the structure the command names.
    CommandBodyTooShort,
    /// A name offset inside the fixed structure, or at or past `cmdsize`.
    NameOffset,
    /// A name with no NUL inside `cmdsize`.
    NameUnterminated,
    /// A name of no bytes.
    NameEmpty,
    /// The commands walked do not add up to `sizeofcmds`.
    SizeofcmdsInconsistent,
}

/// Which cap was passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitKind {
    /// [`MAX_SLICES`].
    Slices,
    /// [`MAX_NCMDS`].
    Commands,
    /// [`MAX_SIZEOFCMDS`].
    CommandBytes,
    /// A shebang's interpreter runs past [`HEAD_BYTES`].
    ShebangLine,
}

/// What this reader does not know, and so cannot answer past.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownKind {
    /// A command with `LC_REQ_DYLD` set that this reader does not name.
    RequiredCommand,
    /// A file type other than the three read and `MH_OBJECT`.
    FileType,
}

/// Why a file cannot be answered for. Each one means `Unchecked` to the caller,
/// and none carries anything from the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachoError {
    /// A structure that does not hold.
    Malformed(MalformedKind),
    /// Over a cap.
    TooLarge(LimitKind),
    /// Something the reader does not know, in a place that forbids skipping it.
    UnknownRequired(UnknownKind),
}

const FAT_MAGIC: u32 = 0xcafe_babe;
const FAT_CIGAM: u32 = 0xbeba_feca;
const FAT_MAGIC_64: u32 = 0xcafe_babf;
const FAT_CIGAM_64: u32 = 0xbfba_feca;
const MH_MAGIC: u32 = 0xfeed_face;
const MH_CIGAM: u32 = 0xcefa_edfe;
const MH_MAGIC_64: u32 = 0xfeed_facf;
const MH_CIGAM_64: u32 = 0xcffa_edfe;

const AR_MAGIC: &[u8] = b"!<arch>\n";
const SHEBANG: &[u8] = b"#!";

const FAT_HEADER_BYTES: u64 = 8;
const FAT_ARCH_BYTES: u64 = 20;
const FAT_ARCH_64_BYTES: u64 = 32;
const MACH_HEADER_BYTES: usize = 28;

const MH_OBJECT: u32 = 0x1;
const MH_EXECUTE: u32 = 0x2;
const MH_DYLIB: u32 = 0x6;
const MH_BUNDLE: u32 = 0x8;

const LC_REQ_DYLD: u32 = 0x8000_0000;
const LC_LOAD_DYLIB: u32 = 0xc;
const LC_ID_DYLIB: u32 = 0xd;
const LC_LOAD_DYLINKER: u32 = 0xe;
const LC_LOAD_WEAK_DYLIB: u32 = 0x8000_0018;
const LC_RPATH: u32 = 0x8000_001c;
const LC_REEXPORT_DYLIB: u32 = 0x8000_001f;
const LC_LAZY_LOAD_DYLIB: u32 = 0x20;
const LC_LOAD_UPWARD_DYLIB: u32 = 0x8000_0023;
const LC_DYLD_ENVIRONMENT: u32 = 0x27;

/// The `LC_REQ_DYLD` commands that carry no reference, from
/// `include/mach-o/loader.h` (R-27 E-9). Any other with the bit set is unknown.
const REQUIRED_WITHOUT_A_REFERENCE: [u32; 4] = [
    0x8000_0022, // LC_DYLD_INFO_ONLY
    0x8000_0028, // LC_MAIN
    0x8000_0033, // LC_DYLD_EXPORTS_TRIE
    0x8000_0034, // LC_DYLD_CHAINED_FIXUPS
];

/// `struct dylib_command`'s size: the name offset, then three version fields.
const DYLIB_COMMAND_BYTES: u32 = 24;
/// `struct rpath_command` and `struct dylinker_command`: the name offset alone.
const STRING_COMMAND_BYTES: u32 = 12;
/// A load command's two fixed fields.
const COMMAND_HEADER_BYTES: usize = 8;
/// Where every command read here keeps its name offset.
const NAME_OFFSET_AT: usize = 8;

#[derive(Debug, Clone, Copy)]
enum Order {
    Big,
    Little,
}

fn read_u32(bytes: &[u8], at: usize, order: Order) -> Option<u32> {
    let end = at.checked_add(4)?;
    let raw: [u8; 4] = bytes.get(at..end)?.try_into().ok()?;
    Some(match order {
        Order::Big => u32::from_be_bytes(raw),
        Order::Little => u32::from_le_bytes(raw),
    })
}

fn read_u64(bytes: &[u8], at: usize, order: Order) -> Option<u64> {
    let end = at.checked_add(8)?;
    let raw: [u8; 8] = bytes.get(at..end)?.try_into().ok()?;
    Some(match order {
        Order::Big => u64::from_be_bytes(raw),
        Order::Little => u64::from_le_bytes(raw),
    })
}

/// True when `given` is exactly what a stage reading the first `wanted` bytes of
/// something `total` long should have received, or more of it.
fn holds_the_prefix(given: &[u8], wanted: usize, total: u64) -> bool {
    let Ok(given_len) = u64::try_from(given.len()) else {
        return false;
    };
    let Ok(wanted) = u64::try_from(wanted) else {
        return false;
    };
    given_len <= total && given_len >= wanted.min(total)
}

const fn malformed(kind: MalformedKind) -> MachoError {
    MachoError::Malformed(kind)
}

/// Stage one. `head` is the file's first [`HEAD_BYTES`], or all of it if it is
/// shorter, and `file_len` its length.
///
/// A file shorter than a magic is [`Classified::NotObject`]. A truncated thin
/// header answers at [`plan_slice`], and a truncated fat table here.
pub fn classify(head: &[u8], file_len: u64) -> Result<Classified, MachoError> {
    if !holds_the_prefix(head, HEAD_BYTES, file_len) {
        return Err(malformed(MalformedKind::HeadInconsistent));
    }
    if head.starts_with(SHEBANG) {
        return shebang(head, file_len).map(Classified::Script);
    }
    if head.starts_with(AR_MAGIC) {
        return Ok(Classified::Skipped(SkippedKind::Archive));
    }
    let Some(magic) = read_u32(head, 0, Order::Big) else {
        return Ok(Classified::NotObject);
    };
    match magic {
        FAT_MAGIC => fat_table(head, file_len, Order::Big, false),
        FAT_MAGIC_64 => fat_table(head, file_len, Order::Big, true),
        FAT_CIGAM => fat_table(head, file_len, Order::Little, false),
        FAT_CIGAM_64 => fat_table(head, file_len, Order::Little, true),
        // The magic was read, so `file_len` is at least its four bytes.
        MH_MAGIC | MH_CIGAM | MH_MAGIC_64 | MH_CIGAM_64 => Ok(Classified::Thin(SliceRange {
            offset: 0,
            size: file_len,
        })),
        _ => Ok(Classified::NotObject),
    }
}

fn fat_table(
    head: &[u8],
    file_len: u64,
    order: Order,
    wide: bool,
) -> Result<Classified, MachoError> {
    let table = malformed(MalformedKind::FatTable);
    let nfat = read_u32(head, 4, order).ok_or(table)?;
    let entry = if wide {
        FAT_ARCH_64_BYTES
    } else {
        FAT_ARCH_BYTES
    };
    // The fit comes before the cap: a Java class's `nfat` is its version number,
    // and a table that cannot fit is the more exact answer than a large one.
    let table_end = u64::from(nfat)
        .checked_mul(entry)
        .and_then(|bytes| bytes.checked_add(FAT_HEADER_BYTES))
        .filter(|end| *end <= file_len)
        .ok_or(table)?;
    if nfat > MAX_SLICES {
        return Err(MachoError::TooLarge(LimitKind::Slices));
    }
    if nfat == 0 {
        return Err(malformed(MalformedKind::FatEmpty));
    }
    let mut ranges = Vec::new();
    for index in 0..nfat {
        let at = u64::from(index)
            .checked_mul(entry)
            .and_then(|bytes| bytes.checked_add(FAT_HEADER_BYTES))
            .and_then(|at| usize::try_from(at).ok())
            .ok_or(table)?;
        // `cputype` and `cpusubtype` first, in either width.
        let (offset, size) = if wide {
            let offset = at.checked_add(8).and_then(|a| read_u64(head, a, order));
            let size = at.checked_add(16).and_then(|a| read_u64(head, a, order));
            (offset, size)
        } else {
            let offset = at.checked_add(8).and_then(|a| read_u32(head, a, order));
            let size = at.checked_add(12).and_then(|a| read_u32(head, a, order));
            (offset.map(u64::from), size.map(u64::from))
        };
        let (offset, size) = offset.zip(size).ok_or(table)?;
        ranges.push(checked_range(offset, size, table_end, file_len)?);
    }
    Ok(Classified::Fat(ranges))
}

fn checked_range(
    offset: u64,
    size: u64,
    table_end: u64,
    file_len: u64,
) -> Result<SliceRange, MachoError> {
    if size == 0 {
        return Err(malformed(MalformedKind::SliceEmpty));
    }
    if offset < table_end {
        return Err(malformed(MalformedKind::SliceOverlapsTable));
    }
    match offset.checked_add(size) {
        Some(end) if end <= file_len => Ok(SliceRange { offset, size }),
        _ => Err(malformed(MalformedKind::SliceBeyondFile)),
    }
}

fn shebang(head: &[u8], file_len: u64) -> Result<Shebang, MachoError> {
    let rest = head.get(SHEBANG.len()..).unwrap_or_default();
    // No line end in what was read, with more of the file past it: its words are not
    // all here. On Darwin a line with no newline in its first 512 bytes does not
    // exec (ENOEXEC, observed with `/bin/echo`, 2026-11-04), so nothing true is lost.
    if !rest.iter().any(|byte| matches!(byte, b'\n' | 0))
        && u64::try_from(head.len()).ok() != Some(file_len)
    {
        return Err(MachoError::TooLarge(LimitKind::ShebangLine));
    }
    let is_blank = |byte: &u8| matches!(byte, b' ' | b'\t');
    // No CR: the kernel keeps it in the path, so a CRLF script names a missing
    // interpreter, and `/x\r/prog` is another directory than `/x` (observed 2026-11-04).
    let is_end = |byte: &u8| matches!(byte, b' ' | b'\t' | b'\n' | 0);
    let start = rest.iter().position(|byte| !is_blank(byte));
    let token = start
        .and_then(|start| rest.get(start..))
        .unwrap_or_default();
    let name = match token.iter().position(is_end) {
        Some(end) => token.get(..end).unwrap_or_default(),
        // The token runs to the end of what was read, and that is the end of
        // the line only if it is the end of the file.
        None if u64::try_from(head.len()).ok() == Some(file_len) => token,
        None => return Err(MachoError::TooLarge(LimitKind::ShebangLine)),
    };
    if name.starts_with(b"/") {
        let leaf = name.rsplit(|byte| *byte == b'/').next().unwrap_or_default();
        return Ok(if leaf.eq_ignore_ascii_case(b"env") {
            Shebang::Env(name.to_vec())
        } else {
            Shebang::Absolute(name.to_vec())
        });
    }
    Ok(Shebang::Relative(name.to_vec()))
}

/// The words of a `#!` line after its interpreter, as bytes, from the first bytes
/// of the file. `env` and the interpreter's own arguments are here, so a caller can
/// test what they name (`#!/usr/bin/env /x/python3`). Empty for a file that is not
/// a script. Only `head` is looked at: the caller reaches this only after [`classify`],
/// which refuses a line with no end in `head` unless `head` is the whole file. A
/// newline or a NUL ends the line, as they do for the kernel (observed with
/// `/bin/echo`, 2026-11-04). A CR does not, so it stays inside the word.
#[must_use]
pub fn shebang_arguments(head: &[u8]) -> Vec<&[u8]> {
    let Some(rest) = head.strip_prefix(SHEBANG) else {
        return Vec::new();
    };
    let line = rest
        .split(|byte| matches!(byte, b'\n' | 0))
        .next()
        .unwrap_or_default();
    line.split(|byte| matches!(byte, b' ' | b'\t'))
        .filter(|word| !word.is_empty())
        .skip(1)
        .collect()
}

/// A Mach-O header, decoded and not yet judged.
#[derive(Debug, Clone, Copy)]
struct Header {
    order: Order,
    /// 28 or 32.
    len: usize,
    cputype: u32,
    file_type: u32,
    ncmds: u32,
    sizeofcmds: u32,
}

fn parse_header(bytes: &[u8]) -> Result<Header, MachoError> {
    let short = malformed(MalformedKind::SliceHeaderShort);
    let magic = read_u32(bytes, 0, Order::Big).ok_or(short)?;
    let (order, len) = match magic {
        MH_MAGIC => (Order::Big, MACH_HEADER_BYTES),
        MH_CIGAM => (Order::Little, MACH_HEADER_BYTES),
        MH_MAGIC_64 => (Order::Big, SLICE_HEADER_BYTES),
        MH_CIGAM_64 => (Order::Little, SLICE_HEADER_BYTES),
        _ => return Err(malformed(MalformedKind::SliceNotMachO)),
    };
    if bytes.len() < len {
        return Err(short);
    }
    Ok(Header {
        order,
        len,
        cputype: read_u32(bytes, 4, order).ok_or(short)?,
        file_type: read_u32(bytes, 12, order).ok_or(short)?,
        ncmds: read_u32(bytes, 16, order).ok_or(short)?,
        sizeofcmds: read_u32(bytes, 20, order).ok_or(short)?,
    })
}

fn check_caps(header: &Header) -> Result<(), MachoError> {
    if header.ncmds > MAX_NCMDS {
        return Err(MachoError::TooLarge(LimitKind::Commands));
    }
    if header.sizeofcmds > MAX_SIZEOFCMDS {
        return Err(MachoError::TooLarge(LimitKind::CommandBytes));
    }
    Ok(())
}

/// How long a window must be: the header plus its commands.
fn window_len(header: &Header) -> Option<u64> {
    u64::try_from(header.len)
        .ok()?
        .checked_add(u64::from(header.sizeofcmds))
}

/// Stage two. `slice_head` is the range's first [`SLICE_HEADER_BYTES`], or all of
/// it if it is shorter, and `slice_size` the range's size.
pub fn plan_slice(slice_head: &[u8], slice_size: u64) -> Result<SlicePlan, MachoError> {
    if !holds_the_prefix(slice_head, SLICE_HEADER_BYTES, slice_size) {
        return Err(malformed(MalformedKind::HeadInconsistent));
    }
    if slice_head.starts_with(AR_MAGIC) {
        return Ok(SlicePlan::Skipped(SkippedKind::Archive));
    }
    let header = parse_header(slice_head)?;
    match header.file_type {
        MH_OBJECT => return Ok(SlicePlan::Skipped(SkippedKind::Object)),
        MH_EXECUTE | MH_DYLIB | MH_BUNDLE => {}
        _ => return Err(MachoError::UnknownRequired(UnknownKind::FileType)),
    }
    check_caps(&header)?;
    match window_len(&header) {
        Some(len) if len <= slice_size => Ok(SlicePlan::Window { len }),
        _ => Err(malformed(MalformedKind::CommandsBeyondSlice)),
    }
}

/// Stage three. `window` is the slice's first `len` bytes, for the `len`
/// [`plan_slice`] answered.
///
/// The header is read again and judged again: the file may have changed between
/// the two reads, and this answer must not rest on the earlier one. A window
/// holding [`SkippedKind::Object`] is an error here, because the caller was told
/// to skip it.
pub fn read_slice(window: &[u8]) -> Result<SliceRefs, MachoError> {
    let header = parse_header(window)?;
    let file_type = match header.file_type {
        MH_EXECUTE => ExaminedType::Execute,
        MH_DYLIB => ExaminedType::Dylib,
        MH_BUNDLE => ExaminedType::Bundle,
        MH_OBJECT => return Err(malformed(MalformedKind::NotExaminedType)),
        _ => return Err(MachoError::UnknownRequired(UnknownKind::FileType)),
    };
    check_caps(&header)?;
    let short = malformed(MalformedKind::WindowShort);
    let sizeofcmds = usize::try_from(header.sizeofcmds).map_err(|_| short)?;
    let end = header.len.checked_add(sizeofcmds).ok_or(short)?;
    let commands = window.get(header.len..end).ok_or(short)?;

    let mut refs = SliceRefs {
        cputype: header.cputype,
        file_type,
        loads: Vec::new(),
        rpaths: Vec::new(),
    };
    let mut at = 0_usize;
    for _ in 0..header.ncmds {
        at = read_command(commands, at, header.order, &mut refs)?;
    }
    if at != commands.len() {
        return Err(malformed(MalformedKind::SizeofcmdsInconsistent));
    }
    Ok(refs)
}

/// Reads the command at `at`, returning where the next one starts.
fn read_command(
    commands: &[u8],
    at: usize,
    order: Order,
    refs: &mut SliceRefs,
) -> Result<usize, MachoError> {
    let beyond = malformed(MalformedKind::CommandBeyondCommands);
    let cmd = read_u32(commands, at, order).ok_or(beyond)?;
    let cmdsize_at = at.checked_add(4).ok_or(beyond)?;
    let cmdsize = read_u32(commands, cmdsize_at, order).ok_or(beyond)?;
    let cmdsize_len = usize::try_from(cmdsize).map_err(|_| beyond)?;
    if cmdsize_len < COMMAND_HEADER_BYTES {
        return Err(malformed(MalformedKind::CommandTooSmall));
    }
    let next = at.checked_add(cmdsize_len).ok_or(beyond)?;
    let body = commands.get(at..next).ok_or(beyond)?;

    match cmd {
        LC_LOAD_DYLIB => push_load(refs, LoadKind::Load, body, DYLIB_COMMAND_BYTES, order)?,
        LC_LOAD_WEAK_DYLIB => push_load(refs, LoadKind::Weak, body, DYLIB_COMMAND_BYTES, order)?,
        LC_REEXPORT_DYLIB => {
            push_load(refs, LoadKind::Reexport, body, DYLIB_COMMAND_BYTES, order)?;
        }
        LC_LAZY_LOAD_DYLIB => push_load(refs, LoadKind::Lazy, body, DYLIB_COMMAND_BYTES, order)?,
        LC_LOAD_UPWARD_DYLIB => {
            push_load(refs, LoadKind::Upward, body, DYLIB_COMMAND_BYTES, order)?;
        }
        LC_LOAD_DYLINKER => {
            push_load(refs, LoadKind::Dylinker, body, STRING_COMMAND_BYTES, order)?;
        }
        LC_DYLD_ENVIRONMENT => push_load(
            refs,
            LoadKind::DyldEnvironment,
            body,
            STRING_COMMAND_BYTES,
            order,
        )?,
        LC_RPATH => refs
            .rpaths
            .push(command_string(body, STRING_COMMAND_BYTES, order)?),
        // A file's own name, not a reference to anything.
        LC_ID_DYLIB => {}
        _ if cmd & LC_REQ_DYLD != 0 && !REQUIRED_WITHOUT_A_REFERENCE.contains(&cmd) => {
            return Err(MachoError::UnknownRequired(UnknownKind::RequiredCommand));
        }
        _ => {}
    }
    Ok(next)
}

fn push_load(
    refs: &mut SliceRefs,
    kind: LoadKind,
    body: &[u8],
    fixed: u32,
    order: Order,
) -> Result<(), MachoError> {
    let name = command_string(body, fixed, order)?;
    refs.loads.push(LoadRef { kind, name });
    Ok(())
}

/// The string a command's name offset points at, cut at the first NUL inside the
/// command. `body` is exactly the command's `cmdsize` bytes.
fn command_string(body: &[u8], fixed: u32, order: Order) -> Result<Vec<u8>, MachoError> {
    let too_short = malformed(MalformedKind::CommandBodyTooShort);
    let bad_offset = malformed(MalformedKind::NameOffset);
    let fixed_len = usize::try_from(fixed).map_err(|_| too_short)?;
    if body.len() < fixed_len {
        return Err(too_short);
    }
    let offset = read_u32(body, NAME_OFFSET_AT, order).ok_or(too_short)?;
    let offset = usize::try_from(offset).map_err(|_| bad_offset)?;
    if offset < fixed_len {
        return Err(bad_offset);
    }
    let tail = body.get(offset..).ok_or(bad_offset)?;
    if tail.is_empty() {
        return Err(bad_offset);
    }
    let end = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(malformed(MalformedKind::NameUnterminated))?;
    let name = tail.get(..end).ok_or(bad_offset)?;
    if name.is_empty() {
        return Err(malformed(MalformedKind::NameEmpty));
    }
    Ok(name.to_vec())
}
