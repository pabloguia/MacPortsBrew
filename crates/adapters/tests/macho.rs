// SPDX-License-Identifier: Apache-2.0

//! The in-process Mach-O reader (`macho`), over specimens built as bytes here.
//! No contaminated binary is committed, and nothing here is ever executed:
//! these are data, and the prefix names in them are only bytes.

use macportsbrew_adapters::macho::{
    classify, plan_slice, read_slice, Classified, ExaminedType, LimitKind, LoadKind, LoadRef,
    MachoError, MalformedKind, Shebang, SkippedKind, SlicePlan, SliceRange, SliceRefs, UnknownKind,
    HEAD_BYTES, MAX_BYTES_PER_FILE, MAX_NCMDS, MAX_SIZEOFCMDS, MAX_SLICES, SLICE_HEADER_BYTES,
};

const LC_SEGMENT_64: u32 = 0x19;
const LC_LOAD_DYLIB: u32 = 0xc;
const LC_ID_DYLIB: u32 = 0xd;
const LC_LOAD_DYLINKER: u32 = 0xe;
const LC_LOAD_WEAK_DYLIB: u32 = 0x8000_0018;
const LC_RPATH: u32 = 0x8000_001c;
const LC_REEXPORT_DYLIB: u32 = 0x8000_001f;
const LC_LAZY_LOAD_DYLIB: u32 = 0x20;
const LC_DYLD_INFO_ONLY: u32 = 0x8000_0022;
const LC_LOAD_UPWARD_DYLIB: u32 = 0x8000_0023;
const LC_DYLD_ENVIRONMENT: u32 = 0x27;
const LC_MAIN: u32 = 0x8000_0028;
const LC_DYLD_EXPORTS_TRIE: u32 = 0x8000_0033;
const LC_DYLD_CHAINED_FIXUPS: u32 = 0x8000_0034;

const MH_OBJECT: u32 = 0x1;
const MH_EXECUTE: u32 = 0x2;
const MH_CORE: u32 = 0x4;
const MH_DYLIB: u32 = 0x6;
const MH_DYLINKER: u32 = 0x7;
const MH_BUNDLE: u32 = 0x8;
const MH_DSYM: u32 = 0xa;

const CPU_X86_64: u32 = 0x0100_0007;
const CPU_ARM64: u32 = 0x0100_000c;

#[derive(Clone, Copy, PartialEq)]
enum Endian {
    Big,
    Little,
}

fn word(value: u32, endian: Endian) -> [u8; 4] {
    match endian {
        Endian::Big => value.to_be_bytes(),
        Endian::Little => value.to_le_bytes(),
    }
}

/// The fixed bytes of a command whose name follows `fixed` bytes in, padded.
fn string_command(cmd: u32, name: &[u8], fixed: usize, endian: Endian) -> Vec<u8> {
    let unpadded = fixed + name.len() + 1;
    let cmdsize = unpadded.div_ceil(8) * 8;
    let mut out = Vec::new();
    out.extend_from_slice(&word(cmd, endian));
    out.extend_from_slice(&word(u32::try_from(cmdsize).unwrap(), endian));
    out.extend_from_slice(&word(u32::try_from(fixed).unwrap(), endian));
    out.resize(fixed, 0);
    out.extend_from_slice(name);
    out.resize(cmdsize, 0);
    out
}

fn dylib(cmd: u32, name: &[u8]) -> Vec<u8> {
    string_command(cmd, name, 24, Endian::Little)
}

fn string_cmd(cmd: u32, name: &[u8]) -> Vec<u8> {
    string_command(cmd, name, 12, Endian::Little)
}

/// A command with no name: only its two fixed fields, padded to `size`.
fn opaque(cmd: u32, size: u32, endian: Endian) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&word(cmd, endian));
    out.extend_from_slice(&word(size, endian));
    out.resize(usize::try_from(size).unwrap(), 0);
    out
}

fn slice_with(
    endian: Endian,
    wide: bool,
    cputype: u32,
    file_type: u32,
    commands: &[Vec<u8>],
) -> Vec<u8> {
    let body: Vec<u8> = commands.concat();
    let magic: u32 = if wide { 0xfeed_facf } else { 0xfeed_face };
    let mut out = Vec::new();
    out.extend_from_slice(&word(magic, endian));
    out.extend_from_slice(&word(cputype, endian));
    out.extend_from_slice(&word(0, endian));
    out.extend_from_slice(&word(file_type, endian));
    out.extend_from_slice(&word(u32::try_from(commands.len()).unwrap(), endian));
    out.extend_from_slice(&word(u32::try_from(body.len()).unwrap(), endian));
    out.extend_from_slice(&word(0, endian));
    if wide {
        out.extend_from_slice(&word(0, endian));
    }
    out.extend_from_slice(&body);
    out
}

/// A little-endian, 64-bit slice: this machine's shape.
fn slice(file_type: u32, commands: &[Vec<u8>]) -> Vec<u8> {
    slice_with(Endian::Little, true, CPU_X86_64, file_type, commands)
}

/// A 32-bit fat file holding `slices` end to end after its table.
fn fat(slices: &[Vec<u8>]) -> Vec<u8> {
    let count = u32::try_from(slices.len()).unwrap();
    let mut out = Vec::new();
    out.extend_from_slice(&0xcafe_babe_u32.to_be_bytes());
    out.extend_from_slice(&count.to_be_bytes());
    let mut offset = 8 + slices.len() * 20;
    for slice in slices {
        out.extend_from_slice(&CPU_X86_64.to_be_bytes());
        out.extend_from_slice(&0_u32.to_be_bytes());
        out.extend_from_slice(&u32::try_from(offset).unwrap().to_be_bytes());
        out.extend_from_slice(&u32::try_from(slice.len()).unwrap().to_be_bytes());
        out.extend_from_slice(&0_u32.to_be_bytes());
        offset += slice.len();
    }
    for slice in slices {
        out.extend_from_slice(slice);
    }
    out
}

/// The same, with the 64-bit fat magic and table.
fn fat64(slices: &[Vec<u8>]) -> Vec<u8> {
    let count = u32::try_from(slices.len()).unwrap();
    let mut out = Vec::new();
    out.extend_from_slice(&0xcafe_babf_u32.to_be_bytes());
    out.extend_from_slice(&count.to_be_bytes());
    let mut offset = 8 + slices.len() * 32;
    for slice in slices {
        out.extend_from_slice(&CPU_X86_64.to_be_bytes());
        out.extend_from_slice(&0_u32.to_be_bytes());
        out.extend_from_slice(&u64::try_from(offset).unwrap().to_be_bytes());
        out.extend_from_slice(&u64::try_from(slice.len()).unwrap().to_be_bytes());
        out.extend_from_slice(&0_u32.to_be_bytes());
        out.extend_from_slice(&0_u32.to_be_bytes());
        offset += slice.len();
    }
    for slice in slices {
        out.extend_from_slice(slice);
    }
    out
}

/// One slice's answer, from the two stages a caller runs on it.
#[derive(Debug, PartialEq)]
enum Answer {
    Skipped(SkippedKind),
    Refs(SliceRefs),
}

fn read_range(file: &[u8], range: SliceRange) -> Result<Answer, MachoError> {
    let start = usize::try_from(range.offset).unwrap();
    let size = usize::try_from(range.size).unwrap();
    let whole = &file[start..start + size];
    let head = &whole[..whole.len().min(SLICE_HEADER_BYTES)];
    match plan_slice(head, range.size)? {
        SlicePlan::Skipped(kind) => Ok(Answer::Skipped(kind)),
        SlicePlan::Window { len } => {
            let window = &whole[..usize::try_from(len).unwrap()];
            read_slice(window).map(Answer::Refs)
        }
    }
}

#[derive(Debug, PartialEq)]
enum File {
    NotObject,
    Skipped(SkippedKind),
    Script(Shebang),
    Slices(Vec<Answer>),
}

/// The whole of what a later caller does, by windows, over bytes in memory.
fn read_file(file: &[u8]) -> Result<File, MachoError> {
    let head = &file[..file.len().min(HEAD_BYTES)];
    match classify(head, u64::try_from(file.len()).unwrap())? {
        Classified::NotObject => Ok(File::NotObject),
        Classified::Skipped(kind) => Ok(File::Skipped(kind)),
        Classified::Script(shebang) => Ok(File::Script(shebang)),
        Classified::Thin(range) => Ok(File::Slices(vec![read_range(file, range)?])),
        Classified::Fat(ranges) => ranges
            .into_iter()
            .map(|range| read_range(file, range))
            .collect::<Result<Vec<_>, _>>()
            .map(File::Slices),
    }
}

fn only_refs(file: &[u8]) -> SliceRefs {
    match read_file(file).unwrap() {
        File::Slices(mut slices) => {
            assert_eq!(slices.len(), 1);
            match slices.remove(0) {
                Answer::Refs(refs) => refs,
                other @ Answer::Skipped(_) => panic!("expected references, got {other:?}"),
            }
        }
        other => panic!("expected one slice, got {other:?}"),
    }
}

fn load(kind: LoadKind, name: &[u8]) -> LoadRef {
    LoadRef {
        kind,
        name: name.to_vec(),
    }
}

fn every_kind() -> Vec<Vec<u8>> {
    vec![
        dylib(LC_LOAD_DYLIB, b"/a/load.dylib"),
        dylib(LC_LOAD_WEAK_DYLIB, b"/a/weak.dylib"),
        dylib(LC_REEXPORT_DYLIB, b"/a/reexport.dylib"),
        dylib(LC_LAZY_LOAD_DYLIB, b"/a/lazy.dylib"),
        dylib(LC_LOAD_UPWARD_DYLIB, b"/a/upward.dylib"),
        string_cmd(LC_LOAD_DYLINKER, b"/usr/lib/dyld"),
        string_cmd(LC_DYLD_ENVIRONMENT, b"DYLD_LIBRARY_PATH=/a"),
        string_cmd(LC_RPATH, b"@loader_path/../lib"),
    ]
}

fn expected_kinds() -> Vec<LoadRef> {
    vec![
        load(LoadKind::Load, b"/a/load.dylib"),
        load(LoadKind::Weak, b"/a/weak.dylib"),
        load(LoadKind::Reexport, b"/a/reexport.dylib"),
        load(LoadKind::Lazy, b"/a/lazy.dylib"),
        load(LoadKind::Upward, b"/a/upward.dylib"),
        load(LoadKind::Dylinker, b"/usr/lib/dyld"),
        load(LoadKind::DyldEnvironment, b"DYLD_LIBRARY_PATH=/a"),
    ]
}

#[test]
fn every_command_read_answers_with_its_kind() {
    let refs = only_refs(&slice(MH_EXECUTE, &every_kind()));
    assert_eq!(refs.loads, expected_kinds());
    assert_eq!(refs.rpaths, vec![b"@loader_path/../lib".to_vec()]);
    assert_eq!(refs.file_type, ExaminedType::Execute);
    assert_eq!(refs.cputype, CPU_X86_64);
}

#[test]
fn every_header_shape_answers_the_same() {
    for endian in [Endian::Big, Endian::Little] {
        for wide in [false, true] {
            let commands: Vec<Vec<u8>> = vec![
                string_command(LC_LOAD_DYLIB, b"/a/load.dylib", 24, endian),
                string_command(LC_RPATH, b"/a/rpath", 12, endian),
            ];
            let refs = only_refs(&slice_with(endian, wide, CPU_ARM64, MH_DYLIB, &commands));
            assert_eq!(refs.loads, vec![load(LoadKind::Load, b"/a/load.dylib")]);
            assert_eq!(refs.rpaths, vec![b"/a/rpath".to_vec()]);
            assert_eq!(refs.cputype, CPU_ARM64);
            assert_eq!(refs.file_type, ExaminedType::Dylib);
        }
    }
}

#[test]
fn a_bundle_is_examined() {
    let refs = only_refs(&slice(MH_BUNDLE, &[dylib(LC_LOAD_DYLIB, b"/a/b")]));
    assert_eq!(refs.file_type, ExaminedType::Bundle);
    assert_eq!(refs.loads.len(), 1);
}

#[test]
fn a_files_own_id_is_never_a_reference_wherever_it_sits() {
    let id = dylib(LC_ID_DYLIB, b"/a/self.dylib");
    let other = dylib(LC_LOAD_DYLIB, b"/a/other.dylib");
    for commands in [
        vec![id.clone(), other.clone()],
        vec![other.clone(), id.clone()],
        vec![other.clone(), id.clone(), other.clone()],
    ] {
        let refs = only_refs(&slice(MH_DYLIB, &commands));
        assert!(
            refs.loads
                .iter()
                .all(|entry| entry.name == b"/a/other.dylib".to_vec()),
            "{refs:?}"
        );
        assert!(!refs.loads.is_empty());
    }
    let alone = only_refs(&slice(MH_DYLIB, &[id]));
    assert!(alone.loads.is_empty() && alone.rpaths.is_empty());
}

#[test]
fn the_slice_this_host_would_not_run_is_still_read() {
    let clean = slice(
        MH_EXECUTE,
        &[dylib(LC_LOAD_DYLIB, b"/usr/lib/libSystem.B.dylib")],
    );
    let crossing = slice_with(
        Endian::Little,
        true,
        CPU_ARM64,
        MH_EXECUTE,
        &[dylib(LC_LOAD_DYLIB, b"/opt/local/lib/libX.dylib")],
    );
    for file in [
        fat(&[clean.clone(), crossing.clone()]),
        fat64(&[clean.clone(), crossing.clone()]),
    ] {
        let File::Slices(slices) = read_file(&file).unwrap() else {
            panic!("expected slices");
        };
        assert_eq!(slices.len(), 2);
        let Answer::Refs(second) = &slices[1] else {
            panic!("expected references");
        };
        assert_eq!(second.cputype, CPU_ARM64);
        assert_eq!(
            second.loads,
            vec![load(LoadKind::Load, b"/opt/local/lib/libX.dylib")]
        );
    }
}

#[test]
fn a_fat_table_written_little_endian_is_read_rather_than_hidden() {
    let inner = slice(MH_EXECUTE, &[dylib(LC_LOAD_DYLIB, b"/a/b")]);
    let mut file = Vec::new();
    file.extend_from_slice(&0xcafe_babe_u32.to_le_bytes());
    file.extend_from_slice(&1_u32.to_le_bytes());
    file.extend_from_slice(&CPU_X86_64.to_le_bytes());
    file.extend_from_slice(&0_u32.to_le_bytes());
    file.extend_from_slice(&28_u32.to_le_bytes());
    file.extend_from_slice(&u32::try_from(inner.len()).unwrap().to_le_bytes());
    file.extend_from_slice(&0_u32.to_le_bytes());
    file.extend_from_slice(&inner);
    let File::Slices(slices) = read_file(&file).unwrap() else {
        panic!("expected slices");
    };
    assert_eq!(slices.len(), 1);
}

#[test]
fn every_truncation_of_a_thin_specimen_is_refused_and_never_panics() {
    let whole = slice(MH_EXECUTE, &every_kind());
    for len in 0..whole.len() {
        let answer = read_file(&whole[..len]);
        if len < 4 {
            assert_eq!(answer, Ok(File::NotObject), "length {len}");
        } else {
            // Any cut loses commands, and that is never a shorter clean list.
            assert!(answer.is_err(), "length {len}: {answer:?}");
        }
    }
    assert!(read_file(&whole).is_ok());
}

#[test]
fn every_truncation_of_a_fat_specimen_is_refused_and_never_panics() {
    let inner = slice(MH_EXECUTE, &every_kind());
    let whole = fat64(&[inner.clone(), inner]);
    for len in 4..whole.len() {
        let answer = read_file(&whole[..len]);
        assert!(answer.is_err(), "length {len}: {answer:?}");
    }
    assert!(read_file(&whole).is_ok());
}

/// Every byte, each set to a few values, through every stage. There is no
/// assertion on the answer: most are errors and some are not. The contract here
/// is that the reader returns, since a panic strands a run.
fn mutate_every_byte(specimen: &[u8]) {
    for at in 0..specimen.len() {
        let original = specimen[at];
        for value in [
            0,
            1,
            7,
            8,
            0x7f,
            0x80,
            0xff,
            original ^ 0x01,
            original ^ 0x80,
        ] {
            let mut copy = specimen.to_vec();
            copy[at] = value;
            let _ = read_file(&copy);
            // The windows on their own, as if the file changed between reads.
            let _ = read_slice(&copy);
            let _ = plan_slice(&copy[..copy.len().min(SLICE_HEADER_BYTES)], 3);
        }
    }
}

#[test]
fn mutating_every_byte_of_a_thin_specimen_never_panics() {
    mutate_every_byte(&slice(MH_DYLIB, &every_kind()));
    mutate_every_byte(&slice_with(
        Endian::Big,
        false,
        CPU_X86_64,
        MH_BUNDLE,
        &[
            string_command(LC_LOAD_DYLIB, b"/a/b", 24, Endian::Big),
            string_command(LC_RPATH, b"/a/c", 12, Endian::Big),
        ],
    ));
}

#[test]
fn mutating_every_byte_of_a_fat_specimen_never_panics() {
    let inner = slice(MH_EXECUTE, &[dylib(LC_LOAD_DYLIB, b"/a/b")]);
    mutate_every_byte(&fat(&[inner.clone(), inner.clone()]));
    mutate_every_byte(&fat64(&[inner]));
}

fn malformed(kind: MalformedKind) -> Result<File, MachoError> {
    Err(MachoError::Malformed(kind))
}

#[test]
fn a_cmdsize_under_eight_is_refused() {
    for size in [0, 4, 7] {
        let file = slice(MH_EXECUTE, &[opaque(LC_LOAD_DYLIB, 8, Endian::Little)]);
        let mut file = file;
        file[32 + 4..32 + 8].copy_from_slice(&u32::to_le_bytes(size));
        assert_eq!(read_file(&file), malformed(MalformedKind::CommandTooSmall));
    }
}

#[test]
fn a_cmdsize_past_the_commands_is_refused() {
    let mut file = slice(MH_EXECUTE, &[dylib(LC_LOAD_DYLIB, b"/a/b")]);
    let past = u32::try_from(file.len()).unwrap();
    file[32 + 4..32 + 8].copy_from_slice(&past.to_le_bytes());
    assert_eq!(
        read_file(&file),
        malformed(MalformedKind::CommandBeyondCommands)
    );
    file[32 + 4..32 + 8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        read_file(&file),
        malformed(MalformedKind::CommandBeyondCommands)
    );
}

#[test]
fn a_command_list_that_does_not_add_up_is_refused() {
    let mut file = slice(MH_EXECUTE, &[dylib(LC_LOAD_DYLIB, b"/a/b")]);
    // Claims no commands, so what follows the header is never walked.
    file[16..20].copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        read_file(&file),
        malformed(MalformedKind::SizeofcmdsInconsistent)
    );
}

#[test]
fn ncmds_beyond_what_sizeofcmds_holds_is_refused() {
    let mut file = slice(MH_EXECUTE, &[dylib(LC_LOAD_DYLIB, b"/a/b")]);
    file[16..20].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(
        read_file(&file),
        malformed(MalformedKind::CommandBeyondCommands)
    );
}

#[test]
fn a_command_smaller_than_its_structure_is_refused() {
    let file = slice(MH_EXECUTE, &[opaque(LC_LOAD_DYLIB, 16, Endian::Little)]);
    assert_eq!(
        read_file(&file),
        malformed(MalformedKind::CommandBodyTooShort)
    );
    let file = slice(MH_EXECUTE, &[opaque(LC_RPATH, 8, Endian::Little)]);
    assert_eq!(
        read_file(&file),
        malformed(MalformedKind::CommandBodyTooShort)
    );
}

fn with_name_bytes(tail: &[u8], offset: u32) -> Vec<u8> {
    let mut command = Vec::new();
    command.extend_from_slice(&LC_LOAD_DYLIB.to_le_bytes());
    let size = u32::try_from(24 + tail.len()).unwrap();
    command.extend_from_slice(&size.to_le_bytes());
    command.extend_from_slice(&offset.to_le_bytes());
    command.resize(24, 0);
    command.extend_from_slice(tail);
    slice(MH_EXECUTE, &[command])
}

#[test]
fn a_name_is_cut_at_the_first_nul_inside_the_command() {
    let refs = only_refs(&with_name_bytes(b"/a/b\0/hidden\0\0\0\0", 24));
    assert_eq!(refs.loads, vec![load(LoadKind::Load, b"/a/b")]);
}

#[test]
fn a_name_with_no_nul_is_refused_rather_than_cut_at_the_command() {
    assert_eq!(
        read_file(&with_name_bytes(b"/opt/loca", 24)),
        malformed(MalformedKind::NameUnterminated)
    );
}

#[test]
fn a_name_cannot_borrow_a_nul_from_the_next_command() {
    // The name ends exactly at `cmdsize`, and the next command opens with a zero.
    let mut first = Vec::new();
    first.extend_from_slice(&LC_LOAD_DYLIB.to_le_bytes());
    first.extend_from_slice(&32_u32.to_le_bytes());
    first.extend_from_slice(&24_u32.to_le_bytes());
    first.resize(24, 0);
    first.extend_from_slice(b"/opt/loc");
    let next = opaque(0x7f, 8, Endian::Little);
    assert_eq!(
        read_file(&slice(MH_EXECUTE, &[first, next])),
        malformed(MalformedKind::NameUnterminated)
    );
}

#[test]
fn an_empty_name_is_refused() {
    assert_eq!(
        read_file(&with_name_bytes(b"\0\0\0\0\0\0\0\0", 24)),
        malformed(MalformedKind::NameEmpty)
    );
}

#[test]
fn a_name_offset_outside_the_string_area_is_refused() {
    let tail = b"/a/b\0\0\0\0";
    for offset in [0, 8, 23, 32, 33, 0x7fff_0000, u32::MAX] {
        assert_eq!(
            read_file(&with_name_bytes(tail, offset)),
            malformed(MalformedKind::NameOffset),
            "offset {offset}"
        );
    }
}

#[test]
fn a_name_stays_bytes() {
    let refs = only_refs(&with_name_bytes(b"/a/\xff\xfe.dylib\0\0\0\0\0", 24));
    assert_eq!(refs.loads[0].name, b"/a/\xff\xfe.dylib".to_vec());
}

#[test]
fn a_fat_range_that_overflows_or_passes_the_file_is_refused() {
    let inner = slice(MH_EXECUTE, &[dylib(LC_LOAD_DYLIB, b"/a/b")]);

    let mut wide = fat64(std::slice::from_ref(&inner));
    wide[8 + 8..8 + 16].copy_from_slice(&(u64::MAX - 4).to_be_bytes());
    wide[8 + 16..8 + 24].copy_from_slice(&64_u64.to_be_bytes());
    assert_eq!(read_file(&wide), malformed(MalformedKind::SliceBeyondFile));

    let mut narrow = fat(std::slice::from_ref(&inner));
    let past = u32::try_from(narrow.len()).unwrap();
    narrow[8 + 12..8 + 16].copy_from_slice(&past.to_be_bytes());
    assert_eq!(
        read_file(&narrow),
        malformed(MalformedKind::SliceBeyondFile)
    );

    let mut empty = fat(std::slice::from_ref(&inner));
    empty[8 + 12..8 + 16].copy_from_slice(&0_u32.to_be_bytes());
    assert_eq!(read_file(&empty), malformed(MalformedKind::SliceEmpty));

    let mut overlapping = fat(&[inner]);
    overlapping[8 + 8..8 + 12].copy_from_slice(&0_u32.to_be_bytes());
    assert_eq!(
        read_file(&overlapping),
        malformed(MalformedKind::SliceOverlapsTable)
    );
}

#[test]
fn a_java_class_shape_is_malformed_never_not_mach_o() {
    // `0xcafebabe`, then a class file's minor and major version, then a body
    // too short for the 52 entries that version reads as.
    let mut class = Vec::new();
    class.extend_from_slice(&0xcafe_babe_u32.to_be_bytes());
    class.extend_from_slice(&0_u16.to_be_bytes());
    class.extend_from_slice(&52_u16.to_be_bytes());
    class.extend_from_slice(&[0; 200]);
    assert_eq!(read_file(&class), malformed(MalformedKind::FatTable));

    assert_eq!(
        read_file(&0xcafe_babe_u32.to_be_bytes()),
        malformed(MalformedKind::FatTable)
    );
    assert_eq!(
        read_file(&0xcafe_babf_u32.to_be_bytes()),
        malformed(MalformedKind::FatTable)
    );
}

#[test]
fn a_fat_table_with_no_slices_is_refused() {
    assert_eq!(read_file(&fat(&[])), malformed(MalformedKind::FatEmpty));
}

#[test]
fn a_slice_that_is_not_mach_o_is_refused_nested_fat_included() {
    let text = b"this slice is plain text and has no magic at all".to_vec();
    assert_eq!(
        read_file(&fat(&[text])),
        malformed(MalformedKind::SliceNotMachO)
    );
    let inner = slice(MH_EXECUTE, &[dylib(LC_LOAD_DYLIB, b"/a/b")]);
    assert_eq!(
        read_file(&fat(&[fat(&[inner])])),
        malformed(MalformedKind::SliceNotMachO)
    );
}

#[test]
fn over_each_cap_is_too_large_and_at_each_cap_is_not() {
    let inner = slice(MH_EXECUTE, &[]);

    let at_cap = fat(&vec![inner.clone(); usize::try_from(MAX_SLICES).unwrap()]);
    assert!(read_file(&at_cap).is_ok());
    let over = fat(&vec![inner; usize::try_from(MAX_SLICES + 1).unwrap()]);
    assert_eq!(
        read_file(&over),
        Err(MachoError::TooLarge(LimitKind::Slices))
    );

    let mut many = slice(MH_EXECUTE, &[]);
    many[16..20].copy_from_slice(&(MAX_NCMDS + 1).to_le_bytes());
    assert_eq!(
        read_file(&many),
        Err(MachoError::TooLarge(LimitKind::Commands))
    );
    let at_cap: Vec<Vec<u8>> = (0..MAX_NCMDS)
        .map(|_| opaque(0x7f, 8, Endian::Little))
        .collect();
    assert!(read_file(&slice(MH_EXECUTE, &at_cap)).is_ok());

    let mut wide_commands = slice(MH_EXECUTE, &[]);
    wide_commands[20..24].copy_from_slice(&(MAX_SIZEOFCMDS + 1).to_le_bytes());
    assert_eq!(
        read_file(&wide_commands),
        Err(MachoError::TooLarge(LimitKind::CommandBytes))
    );
    let exactly = opaque(0x7f, MAX_SIZEOFCMDS, Endian::Little);
    assert!(read_file(&slice(MH_EXECUTE, &[exactly])).is_ok());
}

#[test]
fn a_cap_holds_at_the_last_stage_as_well() {
    let mut many = slice(MH_EXECUTE, &[]);
    many[16..20].copy_from_slice(&(MAX_NCMDS + 1).to_le_bytes());
    assert_eq!(
        read_slice(&many),
        Err(MachoError::TooLarge(LimitKind::Commands))
    );
    let mut wide_commands = slice(MH_EXECUTE, &[]);
    wide_commands[20..24].copy_from_slice(&(MAX_SIZEOFCMDS + 1).to_le_bytes());
    assert_eq!(
        read_slice(&wide_commands),
        Err(MachoError::TooLarge(LimitKind::CommandBytes))
    );
}

#[test]
fn the_bytes_read_for_one_file_are_bounded_by_the_caps() {
    let per_slice = u64::try_from(SLICE_HEADER_BYTES * 2).unwrap() + u64::from(MAX_SIZEOFCMDS);
    assert_eq!(
        MAX_BYTES_PER_FILE,
        u64::try_from(HEAD_BYTES).unwrap() + u64::from(MAX_SLICES) * per_slice
    );
    // The largest table allowed is inside the head, so no second read is needed.
    assert!(8 + usize::try_from(MAX_SLICES).unwrap() * 32 <= HEAD_BYTES);
}

#[test]
fn a_window_never_asks_for_the_rest_of_a_large_file() {
    let body = slice(MH_EXECUTE, &[dylib(LC_LOAD_DYLIB, b"/a/b")]);
    let expected = u64::try_from(body.len()).unwrap();
    let plan = plan_slice(&body[..SLICE_HEADER_BYTES], 200 * 1024 * 1024).unwrap();
    assert_eq!(plan, SlicePlan::Window { len: expected });
    assert_eq!(read_slice(&body).unwrap().loads.len(), 1);
}

#[test]
fn commands_past_the_slice_are_refused_at_the_plan() {
    let mut file = slice(MH_EXECUTE, &[dylib(LC_LOAD_DYLIB, b"/a/b")]);
    file[20..24].copy_from_slice(&1024_u32.to_le_bytes());
    assert_eq!(
        read_file(&file),
        malformed(MalformedKind::CommandsBeyondSlice)
    );
}

#[test]
fn a_header_that_grew_between_the_two_reads_is_refused() {
    let mut file = slice(MH_EXECUTE, &[dylib(LC_LOAD_DYLIB, b"/a/b")]);
    file[20..24].copy_from_slice(&1024_u32.to_le_bytes());
    assert_eq!(
        read_slice(&file),
        Err(MachoError::Malformed(MalformedKind::WindowShort))
    );
}

#[test]
fn the_required_commands_with_no_reference_are_passed_over() {
    let commands = vec![
        opaque(LC_DYLD_INFO_ONLY, 48, Endian::Little),
        opaque(LC_MAIN, 24, Endian::Little),
        opaque(LC_DYLD_EXPORTS_TRIE, 16, Endian::Little),
        opaque(LC_DYLD_CHAINED_FIXUPS, 16, Endian::Little),
        opaque(LC_SEGMENT_64, 72, Endian::Little),
        // Unknown without the bit: dyld may skip it, and so may this.
        opaque(0x7f, 16, Endian::Little),
        dylib(LC_LOAD_DYLIB, b"/a/b"),
    ];
    let refs = only_refs(&slice(MH_EXECUTE, &commands));
    assert_eq!(refs.loads, vec![load(LoadKind::Load, b"/a/b")]);
}

#[test]
fn an_unknown_required_command_answers_unknown() {
    for cmd in [0x8000_007f, 0x8000_0035, 0x8000_0000] {
        let file = slice(MH_EXECUTE, &[opaque(cmd, 16, Endian::Little)]);
        assert_eq!(
            read_file(&file),
            Err(MachoError::UnknownRequired(UnknownKind::RequiredCommand))
        );
    }
}

#[test]
fn an_unknown_file_type_answers_unknown() {
    for file_type in [0, MH_CORE, MH_DYLINKER, MH_DSYM, 0xff] {
        let file = slice(file_type, &[dylib(LC_LOAD_DYLIB, b"/a/b")]);
        assert_eq!(
            read_file(&file),
            Err(MachoError::UnknownRequired(UnknownKind::FileType)),
            "type {file_type}"
        );
        assert_eq!(
            read_slice(&file),
            Err(MachoError::UnknownRequired(UnknownKind::FileType))
        );
    }
}

#[test]
fn an_object_is_skipped_with_its_kind() {
    let object = slice(MH_OBJECT, &[opaque(LC_SEGMENT_64, 72, Endian::Little)]);
    assert_eq!(
        read_file(&object),
        Ok(File::Slices(vec![Answer::Skipped(SkippedKind::Object)]))
    );
    // Whatever its command count: nothing is read from it.
    let mut huge = object.clone();
    huge[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        read_file(&huge),
        Ok(File::Slices(vec![Answer::Skipped(SkippedKind::Object)]))
    );
    assert_eq!(
        read_slice(&object),
        Err(MachoError::Malformed(MalformedKind::NotExaminedType))
    );
}

#[test]
fn an_archive_is_skipped_with_its_kind_alone_or_in_a_fat_wrapper() {
    let archive =
        b"!<arch>\n#1/20           0           0     0     644     4         `\n".to_vec();
    assert_eq!(read_file(&archive), Ok(File::Skipped(SkippedKind::Archive)));
    assert_eq!(
        read_file(&fat(&[archive])),
        Ok(File::Slices(vec![Answer::Skipped(SkippedKind::Archive)]))
    );
}

#[test]
fn what_is_not_an_object_says_so() {
    for bytes in [
        &b""[..],
        &b"a"[..],
        &b"abc"[..],
        &b"plain text"[..],
        &[0x7f, b'E', b'L', b'F', 2, 1, 1, 0][..],
    ] {
        assert_eq!(read_file(bytes), Ok(File::NotObject), "{bytes:?}");
    }
}

#[test]
fn the_bytes_given_must_be_what_the_length_says() {
    let body = slice(MH_EXECUTE, &[]);
    assert_eq!(
        classify(&body, 3),
        Err(MachoError::Malformed(MalformedKind::HeadInconsistent))
    );
    assert_eq!(
        classify(&body[..8], 1 << 30),
        Err(MachoError::Malformed(MalformedKind::HeadInconsistent))
    );
    assert_eq!(
        plan_slice(&body[..8], 1 << 30),
        Err(MachoError::Malformed(MalformedKind::HeadInconsistent))
    );
    assert_eq!(
        plan_slice(&body, 3),
        Err(MachoError::Malformed(MalformedKind::HeadInconsistent))
    );
}

#[test]
fn a_range_too_short_for_its_header_is_refused() {
    let body = slice(MH_EXECUTE, &[]);
    for len in 0..body.len() {
        assert_eq!(
            plan_slice(&body[..len], u64::try_from(len).unwrap()),
            Err(MachoError::Malformed(MalformedKind::SliceHeaderShort)),
            "length {len}"
        );
        assert!(read_slice(&body[..len]).is_err(), "length {len}");
    }
}

fn script(file: &[u8]) -> Shebang {
    match read_file(file).unwrap() {
        File::Script(shebang) => shebang,
        other => panic!("expected a script, got {other:?}"),
    }
}

#[test]
fn a_shebang_answers_its_interpreter_as_bytes() {
    let absolute = |name: &[u8]| Shebang::Absolute(name.to_vec());
    assert_eq!(script(b"#!/bin/sh\necho\n"), absolute(b"/bin/sh"));
    assert_eq!(script(b"#! /bin/sh -e\n"), absolute(b"/bin/sh"));
    assert_eq!(script(b"#!\t/bin/sh\r\n"), absolute(b"/bin/sh"));
    assert_eq!(script(b"#!/bin/sh"), absolute(b"/bin/sh"));
    assert_eq!(
        script(b"#!/opt/local/bin/\xffperl\n"),
        absolute(b"/opt/local/bin/\xffperl")
    );
}

#[test]
fn an_env_interpreter_keeps_its_path_so_the_caller_can_test_it() {
    let env = |name: &[u8]| Shebang::Env(name.to_vec());
    assert_eq!(script(b"#!/usr/bin/env python3\n"), env(b"/usr/bin/env"));
    assert_eq!(
        script(b"#! /usr/bin/env -S python3\n"),
        env(b"/usr/bin/env")
    );
    assert_eq!(script(b"#!/bin/env sh\n"), env(b"/bin/env"));
    assert_eq!(script(b"#!/USR/BIN/ENV sh\n"), env(b"/USR/BIN/ENV"));
    // Under another prefix the path is itself the answer, and it is not lost.
    assert_eq!(
        script(b"#!/opt/local/libexec/gnubin/env python3\n"),
        env(b"/opt/local/libexec/gnubin/env")
    );
    // The last component decides, whole: neither of these runs `env`.
    assert_eq!(
        script(b"#!/opt/local/bin/myenv\n"),
        Shebang::Absolute(b"/opt/local/bin/myenv".to_vec())
    );
    assert_eq!(
        script(b"#!/opt/local/env/perl\n"),
        Shebang::Absolute(b"/opt/local/env/perl".to_vec())
    );
    // Not absolute: nothing to test, and not read as `env`.
    assert_eq!(script(b"#!env sh\n"), Shebang::Relative(b"env".to_vec()));
}

#[test]
fn a_shebang_that_is_not_absolute_says_so() {
    assert_eq!(script(b"#! .\n"), Shebang::Relative(b".".to_vec()));
    assert_eq!(script(b"#!sh\n"), Shebang::Relative(b"sh".to_vec()));
    assert_eq!(script(b"#!"), Shebang::Relative(Vec::new()));
    assert_eq!(script(b"#!  \n"), Shebang::Relative(Vec::new()));
    assert_eq!(script(b"#!\n/bin/sh\n"), Shebang::Relative(Vec::new()));
}

/// The name ends and the line does not, so its words are not all here. The kernel
/// refuses such a line with ENOEXEC; a line it accepts ends in a newline, and a CR is
/// not an end (observed with `/bin/echo`, 2026-11-04).
#[test]
fn a_shebang_line_with_no_end_in_the_head_is_refused_though_its_name_ended() {
    let mut file = b"#!/bin/sh -x ".to_vec();
    file.resize(HEAD_BYTES + 100, b'a');
    assert_eq!(
        read_file(&file),
        Err(MachoError::TooLarge(LimitKind::ShebangLine))
    );
    // The whole file: nothing is past it.
    file.truncate(HEAD_BYTES);
    assert!(matches!(script(&file), Shebang::Absolute(_)));
    // A line end in the head, whatever follows it.
    let mut file = b"#!/bin/sh -x\n".to_vec();
    file.resize(HEAD_BYTES + 100, b'a');
    assert!(matches!(script(&file), Shebang::Absolute(_)));
    assert_eq!(
        macportsbrew_adapters::macho::shebang_arguments(b"#!/bin/sh a\r/x b\nc"),
        vec![&b"a\r/x"[..], b"b"]
    );
    assert_eq!(
        macportsbrew_adapters::macho::shebang_arguments(b"#!/bin/sh a\0b c\n"),
        vec![&b"a"[..]]
    );
}

#[test]
fn a_shebang_longer_than_the_head_is_refused_rather_than_cut() {
    let mut file = b"#!/".to_vec();
    file.resize(HEAD_BYTES + 100, b'a');
    assert_eq!(
        read_file(&file),
        Err(MachoError::TooLarge(LimitKind::ShebangLine))
    );
    // Exactly the head, and that is the file: the token ends with it.
    file.truncate(HEAD_BYTES);
    assert!(matches!(script(&file), Shebang::Absolute(_)));
}

#[test]
fn the_head_stage_reports_ranges_without_reading_the_slices() {
    let inner = slice(MH_EXECUTE, &[]);
    let file = fat(std::slice::from_ref(&inner));
    let head = &file[..8 + 20];
    let declared = u64::try_from(file.len()).unwrap();
    // The caller gave less than a full head of a longer file: inconsistent.
    assert!(classify(head, declared).is_err());
    let expected = SliceRange {
        offset: 28,
        size: u64::try_from(inner.len()).unwrap(),
    };
    assert_eq!(
        classify(&file, declared),
        Ok(Classified::Fat(vec![expected]))
    );
    assert_eq!(
        classify(&inner, u64::try_from(inner.len()).unwrap()),
        Ok(Classified::Thin(SliceRange {
            offset: 0,
            size: u64::try_from(inner.len()).unwrap(),
        }))
    );
}
