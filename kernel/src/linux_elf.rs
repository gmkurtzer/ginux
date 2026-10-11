use core::num::NonZeroU64;

const ELF_HEADER_SIZE: usize = 64;
pub(crate) const PROGRAM_HEADER_SIZE: usize = 56;
const MIN_LOAD_SEGMENT_COUNT: usize = 2;
const MAX_LOAD_SEGMENT_COUNT: usize = 3;
const MAX_PROGRAM_HEADER_COUNT: usize = 9;
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const PT_PHDR: u32 = 6;
const PT_TLS: u32 = 7;
const PT_GNU_STACK: u32 = 0x6474_e551;
const PT_GNU_RELRO: u32 = 0x6474_e552;
const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;
const EM_X86_64: u16 = 62;
const PF_EXECUTE: u32 = 1;
const PF_WRITE: u32 = 2;
const PF_READ: u32 = 4;
const MIN_USER_ADDRESS: u64 = 0x1_0000;
const MAX_USER_ADDRESS: u64 = 1 << 47;
const PAGE_SIZE: usize = 4096;
const MAX_EXECUTABLE_SEGMENT_PAGES: usize = 5;
const MAX_OTHER_SEGMENT_PAGES: usize = 2;
const MAX_INTERPRETER_PATH_SIZE: usize = 256;
const MAX_TLS_TEMPLATE_SIZE: u64 = 64;
const FIXTURE_EXECUTABLE_MEMORY_SIZE: u64 = (5 * PAGE_SIZE) as u64;
const FIXTURE_DATA_OFFSET: usize = 0x4000;
const FIXTURE_SIZE: usize = FIXTURE_DATA_OFFSET + 8;
const INTERPRETED_DATA_OFFSET: usize = 0x4100;
const INTERPRETED_FIXTURE_SIZE: usize =
    INTERPRETED_DATA_OFFSET + 8 + 13 * DYNAMIC_ENTRY_SIZE + 2 * SYMBOL_ENTRY_SIZE + 28 + 20;
const DYNAMIC_ENTRY_SIZE: usize = 16;
const RELA_ENTRY_SIZE: usize = 24;
const SYMBOL_ENTRY_SIZE: usize = 24;
const DT_NULL: u64 = 0;
const DT_NEEDED: u64 = 1;
const DT_PLTRELSZ: u64 = 2;
const DT_PLTGOT: u64 = 3;
const DT_HASH: u64 = 4;
const DT_STRTAB: u64 = 5;
const DT_SYMTAB: u64 = 6;
const DT_RELA: u64 = 7;
const DT_RELASZ: u64 = 8;
const DT_RELAENT: u64 = 9;
const DT_STRSZ: u64 = 10;
const DT_SYMENT: u64 = 11;
const DT_INIT: u64 = 12;
const DT_SONAME: u64 = 14;
const DT_PLTREL: u64 = 20;
const DT_DEBUG: u64 = 21;
const DT_JMPREL: u64 = 23;
const DT_FLAGS_1: u64 = 0x6fff_fffb;
const DT_RELACOUNT: u64 = 0x6fff_fff9;
const DT_INIT_ARRAY: u64 = 25;
const DT_INIT_ARRAYSZ: u64 = 27;
const DT_FLAGS: u64 = 30;
const DT_PREINIT_ARRAY: u64 = 32;
const DT_PREINIT_ARRAYSZ: u64 = 33;
const DT_GNU_HASH: u64 = 0x6fff_fef5;
const DF_1_PIE: u64 = 0x0800_0000;
const DF_STATIC_TLS: u64 = 0x10;
const R_X86_64_GLOB_DAT: u32 = 6;
const R_X86_64_JUMP_SLOT: u32 = 7;
const R_X86_64_RELATIVE: u64 = 8;
const R_X86_64_TPOFF64: u32 = 18;
const PROBE_CODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/linux-probe.bin"));
const INTERPRETER_CODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/linux-interpreter.bin"));
const DEPENDENCY_CODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/linux-dependency.bin"));
const LINKED_PROBE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/linux-probe.elf"));
const LINKED_DYNAMIC_GRAPH_MAIN: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/linux-linked-graph-main.elf"));
const LINKED_DYNAMIC_DEPENDENCY: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/libginux.so"));
const LINKED_DYNAMIC_AUXILIARY: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/libginux-aux.so"));
const LINKED_DYNAMIC_INTERPRETER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ld-ginux.so"));

pub(crate) fn runtime_main_bytes() -> &'static [u8] {
    LINKED_DYNAMIC_GRAPH_MAIN
}

pub(crate) fn verify_dependency_list_metadata() -> bool {
    const INTERPRETER_PATH: &[u8] = b"/lib64/ld-ginux.so";
    const PRIMARY: &[u8] = b"libginux.so";
    const AUXILIARY: &[u8] = b"libginux-aux.so";
    let Some(mut main) = parse(LINKED_DYNAMIC_GRAPH_MAIN) else {
        crate::serial_write_bytes(b"\r\nginux: dependency-list main parse failed");
        return false;
    };
    let Some(primary) = parse(LINKED_DYNAMIC_DEPENDENCY) else {
        crate::serial_write_bytes(b"\r\nginux: primary dependency parse failed");
        return false;
    };
    let Some(auxiliary) = parse(LINKED_DYNAMIC_AUXILIARY) else {
        crate::serial_write_bytes(b"\r\nginux: auxiliary dependency parse failed");
        return false;
    };
    let interpreter_authority = InterpreterAuthority {
        path: INTERPRETER_PATH,
        sha256: crate::crypto::sha256(LINKED_DYNAMIC_INTERPRETER),
    };
    let dependency_authorities = [
        DependencyAuthority {
            name: PRIMARY,
            sha256: crate::crypto::sha256(LINKED_DYNAMIC_DEPENDENCY),
            load_bias: 0x61_0000,
        },
        DependencyAuthority {
            name: AUXILIARY,
            sha256: crate::crypto::sha256(LINKED_DYNAMIC_AUXILIARY),
            load_bias: 0x63_0000,
        },
    ];
    let Some(plan) = plan_authenticated_pie_dependency_graph(
        &main,
        0x3f_f000,
        &interpreter_authority,
        LINKED_DYNAMIC_INTERPRETER,
        0x60_0000,
        &dependency_authorities,
        [LINKED_DYNAMIC_DEPENDENCY, LINKED_DYNAMIC_AUXILIARY],
    ) else {
        crate::serial_write_bytes(b"\r\nginux: dependency graph authentication failed");
        return false;
    };
    let mut wrong_digest_authorities = dependency_authorities;
    wrong_digest_authorities[1].sha256[0] ^= 1;
    let mut overlapping_authorities = dependency_authorities;
    overlapping_authorities[1].load_bias = dependency_authorities[0].load_bias;
    let reversed_authorities = [dependency_authorities[1], dependency_authorities[0]];
    let main_dynamic = main.dynamic_relocations;
    let mut missing_preinit = main_dynamic.unwrap();
    missing_preinit.preinit_array = None;
    missing_preinit.preinit_relocation_target = None;
    missing_preinit.preinit_relocation_addend = None;
    main.dynamic_relocations = Some(missing_preinit);
    let missing_preinit_rejected = plan_authenticated_pie_dependency_graph(
        &main,
        0x3f_f000,
        &interpreter_authority,
        LINKED_DYNAMIC_INTERPRETER,
        0x60_0000,
        &dependency_authorities,
        [LINKED_DYNAMIC_DEPENDENCY, LINKED_DYNAMIC_AUXILIARY],
    )
    .is_none();
    main.dynamic_relocations = main_dynamic;
    let primary_tls = primary.tls;
    let valid = main.needed_libraries == [Some(PRIMARY), Some(AUXILIARY)]
        && main.dynamic_symbol == Some(b"ginux_dependency_symbol")
        && main.secondary_dynamic_symbol == Some(b"ginux_auxiliary_symbol")
        && main.dynamic_relocations.is_some_and(|dynamic| {
            dynamic.relocation_count == 6
                && !dynamic.symbol_defined
                && !dynamic.secondary_symbol_defined
                && dynamic.init_function.is_some()
                && dynamic.preinit_array == dynamic.preinit_relocation_target
                && dynamic.preinit_relocation_addend.is_some()
                && dynamic.init_array == Some(dynamic.relocation_target)
                && dynamic.tls_relocation_target.is_none()
                && dynamic
                    .symbol_relocation
                    .is_some_and(|relocation| relocation.symbol_index == 1)
                && dynamic
                    .jump_slot_relocation
                    .is_some_and(|relocation| relocation.symbol_index == 1)
                && dynamic
                    .secondary_symbol_relocation
                    .is_some_and(|relocation| relocation.symbol_index == 2)
                && dynamic
                    .secondary_jump_slot_relocation
                    .is_some_and(|relocation| relocation.symbol_index == 2)
        })
        && primary.program_header_count == 6
        && primary.dynamic_relocations.is_some_and(|dynamic| {
            dynamic.relocation_count == 4 && dynamic.tls_relocation_target.is_some()
        })
        && primary_tls.is_some_and(|template| {
            template.bytes == b"DSOTLS!\n"
                && template.memory_size == 16
                && template.alignment == 8
                && plan.dependencies[0].tls.is_some_and(|relocated| {
                    relocated.bytes == template.bytes
                        && relocated.file_offset == template.file_offset
                        && relocated.virtual_address
                            == template
                                .virtual_address
                                .checked_add(dependency_authorities[0].load_bias)
                                .unwrap_or(0)
                        && relocated.memory_size == template.memory_size
                        && relocated.alignment == template.alignment
                })
        })
        && auxiliary.soname == Some(AUXILIARY)
        && auxiliary.program_header_count == 5
        && auxiliary.tls.is_some_and(|template| {
            template.bytes == b"AUXTLS!\n"
                && template.memory_size == 16
                && template.alignment == 8
                && plan.dependencies[1].tls.is_some_and(|relocated| {
                    relocated.bytes == template.bytes
                        && relocated.file_offset == template.file_offset
                        && relocated.virtual_address
                            == template
                                .virtual_address
                                .checked_add(dependency_authorities[1].load_bias)
                                .unwrap_or(0)
                        && relocated.memory_size == template.memory_size
                        && relocated.alignment == template.alignment
                })
        })
        && auxiliary.needed_libraries == [None; 2]
        && auxiliary.dynamic_symbol == Some(b"ginux_auxiliary_symbol")
        && auxiliary.dynamic_relocations.is_some_and(|dynamic| {
            dynamic.relocation_count == 4
                && dynamic.symbol_defined
                && dynamic.init_function.is_some()
                && dynamic.preinit_array.is_none()
                && dynamic.symbol_relocation.is_some()
                && dynamic.jump_slot_relocation.is_some()
                && dynamic.init_array == Some(dynamic.relocation_target)
                && dynamic.tls_relocation_target.is_some()
        })
        && plan.dependency_bases == [0x61_0000, 0x63_0000]
        && plan.process.main_entry == 0x40_0100
        && plan.dependencies[0].entry == 0x61_0100
        && plan.dependencies[1].entry == 0x63_0100
        && missing_preinit_rejected
        && plan_authenticated_pie_dependency_graph(
            &main,
            0x3f_f000,
            &interpreter_authority,
            LINKED_DYNAMIC_INTERPRETER,
            0x60_0000,
            &wrong_digest_authorities,
            [LINKED_DYNAMIC_DEPENDENCY, LINKED_DYNAMIC_AUXILIARY],
        )
        .is_none()
        && plan_authenticated_pie_dependency_graph(
            &main,
            0x3f_f000,
            &interpreter_authority,
            LINKED_DYNAMIC_INTERPRETER,
            0x60_0000,
            &overlapping_authorities,
            [LINKED_DYNAMIC_DEPENDENCY, LINKED_DYNAMIC_AUXILIARY],
        )
        .is_none()
        && plan_authenticated_pie_dependency_graph(
            &main,
            0x3f_f000,
            &interpreter_authority,
            LINKED_DYNAMIC_INTERPRETER,
            0x60_0000,
            &reversed_authorities,
            [LINKED_DYNAMIC_AUXILIARY, LINKED_DYNAMIC_DEPENDENCY],
        )
        .is_none();
    if !valid {
        crate::serial_write_bytes(b"\r\nginux: auxiliary dependency metadata mismatch");
    }
    valid
}

pub(crate) fn runtime_interpreter_bytes() -> &'static [u8] {
    LINKED_DYNAMIC_INTERPRETER
}

pub(crate) fn runtime_dependency_bytes() -> &'static [u8] {
    LINKED_DYNAMIC_DEPENDENCY
}

pub(crate) fn runtime_auxiliary_dependency_bytes() -> &'static [u8] {
    LINKED_DYNAMIC_AUXILIARY
}

pub(crate) fn verify_tls_template_metadata() -> bool {
    let Some(main) = parse(LINKED_DYNAMIC_GRAPH_MAIN) else {
        return false;
    };
    let Some(template) = main.tls else {
        return false;
    };
    let Some(plan) = plan_position_independent_at(&main, 0x3f_f000, true) else {
        return false;
    };
    let linked_valid = main.program_header_count == 9
        && template.bytes == b"TLS_OK!\n"
        && template.memory_size == 16
        && template.alignment == 8
        && plan.tls.is_some_and(|relocated| {
            relocated.bytes == template.bytes
                && relocated.file_offset == template.file_offset
                && relocated.virtual_address
                    == template.virtual_address.checked_add(0x3f_f000).unwrap_or(0)
                && relocated.memory_size == template.memory_size
                && relocated.alignment == template.alignment
        });

    let mut header = [0_u8; 80];
    header[64..72].copy_from_slice(b"GINUXTLS");
    write_u32(&mut header, 0, PT_TLS);
    write_u32(&mut header, 4, PF_READ);
    write_u64(&mut header, 8, 64);
    write_u64(&mut header, 16, 0x5040);
    write_u64(&mut header, 24, 0x5040);
    write_u64(&mut header, 32, 8);
    write_u64(&mut header, 40, 16);
    write_u64(&mut header, 48, 8);
    let Some(valid_template) = parse_tls(&header, 0) else {
        return false;
    };
    let writable = Segment {
        bytes: &header,
        file_offset: 0,
        virtual_address: 0x5000,
        memory_size: 0x100,
        flags: PF_READ | PF_WRITE,
    };
    let mut malformed = header;
    write_u32(&mut malformed, 4, PF_READ | PF_WRITE);
    let bad_flags_rejected = parse_tls(&malformed, 0).is_none();
    malformed = header;
    write_u64(&mut malformed, 32, 0);
    let empty_template_rejected = parse_tls(&malformed, 0).is_none();
    malformed = header;
    write_u64(&mut malformed, 40, MAX_TLS_TEMPLATE_SIZE + 1);
    let oversized_template_rejected = parse_tls(&malformed, 0).is_none();
    malformed = header;
    write_u64(&mut malformed, 48, 3);
    let bad_alignment_rejected = parse_tls(&malformed, 0).is_none();
    let misplaced_template = TlsTemplate {
        virtual_address: valid_template.virtual_address + 8,
        ..valid_template
    };

    linked_valid
        && tls_within_writable(valid_template, writable)
        && !tls_within_writable(misplaced_template, writable)
        && bad_flags_rejected
        && empty_template_rejected
        && oversized_template_rejected
        && bad_alignment_rejected
}

pub(crate) fn verify_tls_relocation_metadata() -> bool {
    let mut record = [0_u8; RELA_ENTRY_SIZE];
    write_u64(&mut record, 0, 0x4020);
    write_u64(&mut record, 8, R_X86_64_TPOFF64 as u64);
    let valid = parse_tls_relocation(&record, 0x4000, 0x4040, 0x4000)
        .is_some_and(|target| target.get() == 0x4020);

    let rejected = |offset: usize, value: u64| {
        let mut malformed = record;
        write_u64(&mut malformed, offset, value);
        parse_tls_relocation(&malformed, 0x4000, 0x4040, 0x4000).is_none()
    };
    valid
        && rejected(8, 17)
        && rejected(8, (1_u64 << 32) | R_X86_64_TPOFF64 as u64)
        && rejected(16, 1)
        && rejected(0, 0x4000)
        && rejected(0, 0x401f)
        && rejected(0, 0x3ff8)
        && rejected(0, 0x4040)
}

pub(crate) fn verify_static_tls_layout() -> bool {
    let template = |bytes: &'static [u8], memory_size, alignment| TlsTemplate {
        bytes,
        file_offset: 0,
        virtual_address: 0,
        memory_size,
        alignment,
    };
    let main = template(b"MAIN", 16, 8);
    let primary = template(b"PRIMARY", 16, 8);
    let auxiliary = template(b"AUX", 16, 8);
    let Some(layout) = plan_static_tls_layout(main, [primary, auxiliary], 4032, 16, 4096) else {
        return false;
    };
    let mixed_main = template(b"M", 9, 16);
    let mixed_primary = template(b"P", 17, 32);
    let mixed_auxiliary = template(b"A", 1, 8);
    let Some(mixed) =
        plan_static_tls_layout(mixed_main, [mixed_primary, mixed_auxiliary], 4032, 16, 4096)
    else {
        return false;
    };
    let Some(page_layout) = plan_static_tls_layout(main, [primary, auxiliary], 192, 16, 256) else {
        return false;
    };
    let mut page = [0xaa_u8; 256];
    let page_materialized = materialize_static_tls_page(
        page_layout,
        main,
        [primary, auxiliary],
        0x1234_5678,
        &mut page,
    ) && page[176..180] == *b"MAIN"
        && page[160..167] == *b"PRIMARY"
        && page[144..147] == *b"AUX"
        && page[147..160].iter().all(|byte| *byte == 0)
        && page[167..176].iter().all(|byte| *byte == 0)
        && page[180..192].iter().all(|byte| *byte == 0)
        && page[192..200] == 0x1234_5678_u64.to_le_bytes()
        && page[200..208].iter().all(|byte| *byte == 0)
        && page[..144].iter().all(|byte| *byte == 0)
        && page[208..].iter().all(|byte| *byte == 0);
    let mut invalid_layout = page_layout;
    invalid_layout.main.start_offset += 8;
    let mut unchanged = [0xaa_u8; 256];
    let invalid_unchanged = !materialize_static_tls_page(
        invalid_layout,
        main,
        [primary, auxiliary],
        0x1234_5678,
        &mut unchanged,
    ) && unchanged.iter().all(|byte| *byte == 0xaa);
    layout.main.start_offset == 4016
        && layout.main.thread_offset == (-16_i64) as u64
        && layout.dependencies[0].start_offset == 4000
        && layout.dependencies[0].thread_offset == (-32_i64) as u64
        && layout.dependencies[1].start_offset == 3984
        && layout.dependencies[1].thread_offset == (-48_i64) as u64
        && layout.extent_start == 3984
        && layout.extent_size == 64
        && mixed.main.start_offset == 4016
        && mixed.dependencies[0].start_offset == 3968
        && mixed.dependencies[1].start_offset == 3960
        && mixed.extent_size == 88
        && page_materialized
        && invalid_unchanged
        && plan_static_tls_layout(template(b"XX", 1, 8), [primary, auxiliary], 4032, 16, 4096)
            .is_none()
        && plan_static_tls_layout(template(b"M", 16, 3), [primary, auxiliary], 4032, 16, 4096)
            .is_none()
        && plan_static_tls_layout(template(b"", 0, 8), [primary, auxiliary], 4032, 16, 4096)
            .is_none()
        && plan_static_tls_layout(main, [template(b"P", 4096, 8), auxiliary], 4032, 16, 4096)
            .is_none()
        && plan_static_tls_layout(main, [primary, auxiliary], 4031, 16, 4096).is_none()
        && plan_static_tls_layout(main, [primary, auxiliary], 4032, 65, 4096).is_none()
}

#[derive(Clone, Copy)]
pub(crate) struct Segment<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) file_offset: u64,
    pub(crate) virtual_address: u64,
    pub(crate) memory_size: u64,
    pub(crate) flags: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct TlsTemplate<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) file_offset: u64,
    pub(crate) virtual_address: u64,
    pub(crate) memory_size: u64,
    pub(crate) alignment: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct StaticTlsBlock {
    pub(crate) start_offset: usize,
    pub(crate) memory_size: usize,
    pub(crate) alignment: usize,
    pub(crate) thread_offset: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct StaticTlsLayout {
    pub(crate) main: StaticTlsBlock,
    pub(crate) dependencies: [StaticTlsBlock; 2],
    pub(crate) extent_start: usize,
    pub(crate) extent_size: usize,
    pub(crate) thread_pointer_offset: usize,
    pub(crate) tcb_size: usize,
}

pub(crate) fn plan_static_tls_layout(
    main: TlsTemplate<'_>,
    dependencies: [TlsTemplate<'_>; 2],
    thread_pointer_offset: usize,
    tcb_size: usize,
    page_size: usize,
) -> Option<StaticTlsLayout> {
    let describe = |template: TlsTemplate<'_>| {
        let memory_size = usize::try_from(template.memory_size).ok()?;
        let alignment = usize::try_from(template.alignment).ok()?;
        if memory_size == 0
            || template.bytes.len() > memory_size
            || alignment == 0
            || !alignment.is_power_of_two()
        {
            return None;
        }
        let block_size = memory_size
            .checked_add(alignment - 1)
            .map(|size| size & !(alignment - 1))?;
        Some((memory_size, alignment, block_size))
    };
    let main_description = describe(main)?;
    let dependency_descriptions = [describe(dependencies[0])?, describe(dependencies[1])?];
    let maximum_alignment = dependency_descriptions
        .iter()
        .fold(main_description.1, |maximum, block| maximum.max(block.1));
    if thread_pointer_offset > page_size
        || tcb_size < 16
        || thread_pointer_offset.checked_add(tcb_size)? > page_size
        || !thread_pointer_offset.is_multiple_of(maximum_alignment)
    {
        return None;
    }
    let place = |cursor: usize, description: (usize, usize, usize)| {
        cursor
            .checked_sub(description.2)
            .map(|start| start & !(description.1 - 1))
    };
    let main_start = place(thread_pointer_offset, main_description)?;
    let first_start = place(main_start, dependency_descriptions[0])?;
    let second_start = place(first_start, dependency_descriptions[1])?;
    let block = |start: usize, description: (usize, usize, usize)| StaticTlsBlock {
        start_offset: start,
        memory_size: description.0,
        alignment: description.1,
        thread_offset: 0_u64.wrapping_sub((thread_pointer_offset - start) as u64),
    };
    Some(StaticTlsLayout {
        main: block(main_start, main_description),
        dependencies: [
            block(first_start, dependency_descriptions[0]),
            block(second_start, dependency_descriptions[1]),
        ],
        extent_start: second_start,
        extent_size: thread_pointer_offset
            .checked_sub(second_start)?
            .checked_add(tcb_size)?,
        thread_pointer_offset,
        tcb_size,
    })
}

pub(crate) fn materialize_static_tls_page(
    layout: StaticTlsLayout,
    main: TlsTemplate<'_>,
    dependencies: [TlsTemplate<'_>; 2],
    thread_pointer: u64,
    page: &mut [u8],
) -> bool {
    if plan_static_tls_layout(
        main,
        dependencies,
        layout.thread_pointer_offset,
        layout.tcb_size,
        page.len(),
    ) != Some(layout)
    {
        return false;
    }
    page.fill(0);
    let copy = |page: &mut [u8], block: StaticTlsBlock, template: TlsTemplate<'_>| -> Option<()> {
        page.get_mut(block.start_offset..block.start_offset.checked_add(template.bytes.len())?)?
            .copy_from_slice(template.bytes);
        Some(())
    };
    if copy(page, layout.dependencies[1], dependencies[1]).is_none()
        || copy(page, layout.dependencies[0], dependencies[0]).is_none()
        || copy(page, layout.main, main).is_none()
    {
        return false;
    }
    let Some(tcb_end) = layout.thread_pointer_offset.checked_add(layout.tcb_size) else {
        return false;
    };
    let Some(tcb) = page.get_mut(layout.thread_pointer_offset..tcb_end) else {
        return false;
    };
    tcb[..8].copy_from_slice(&thread_pointer.to_le_bytes());
    true
}

pub(crate) struct Image<'a> {
    pub(crate) kind: ImageKind,
    pub(crate) entry: u64,
    pub(crate) program_headers: u64,
    pub(crate) program_header_count: u16,
    pub(crate) executable: Segment<'a>,
    pub(crate) read_only: Option<Segment<'a>>,
    pub(crate) writable: Segment<'a>,
    pub(crate) relro: Option<Relro>,
    pub(crate) tls: Option<TlsTemplate<'a>>,
    pub(crate) interpreter: Option<&'a [u8]>,
    pub(crate) dynamic_relocations: Option<DynamicRelocations>,
    pub(crate) dynamic_symbol: Option<&'a [u8]>,
    pub(crate) secondary_dynamic_symbol: Option<&'a [u8]>,
    pub(crate) needed_libraries: [Option<&'a [u8]>; 2],
    pub(crate) soname: Option<&'a [u8]>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageKind {
    Executable,
    PositionIndependent,
}

#[derive(Clone, Copy)]
pub(crate) struct Relro {
    pub(crate) virtual_address: u64,
    pub(crate) memory_size: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct DynamicRelocations {
    pub(crate) table_address: u64,
    pub(crate) relocation_address: u64,
    pub(crate) jump_relocation_address: Option<NonZeroU64>,
    pub(crate) relocation_count: usize,
    pub(crate) relocation_target: u64,
    pub(crate) relocation_addend: u64,
    pub(crate) symbol_table_address: Option<u64>,
    pub(crate) string_table_address: Option<u64>,
    pub(crate) string_table_size: usize,
    pub(crate) hash_table_address: Option<u64>,
    pub(crate) hash_is_gnu: bool,
    pub(crate) init_function: Option<u64>,
    pub(crate) preinit_array: Option<u64>,
    pub(crate) preinit_relocation_target: Option<u64>,
    pub(crate) preinit_relocation_addend: Option<u64>,
    pub(crate) init_array: Option<u64>,
    pub(crate) tls_relocation_target: Option<NonZeroU64>,
    pub(crate) symbol_relocation: Option<SymbolRelocation>,
    pub(crate) jump_slot_relocation: Option<SymbolRelocation>,
    pub(crate) secondary_symbol_relocation: Option<SymbolRelocation>,
    pub(crate) secondary_jump_slot_relocation: Option<SymbolRelocation>,
    pub(crate) symbol_defined: bool,
    pub(crate) secondary_symbol_defined: bool,
}

#[derive(Clone, Copy)]
struct DynamicMetadata<'a> {
    relocations: DynamicRelocations,
    symbol_name: Option<&'a [u8]>,
    secondary_symbol_name: Option<&'a [u8]>,
    needed_libraries: [Option<&'a [u8]>; 2],
    soname: Option<&'a [u8]>,
}

#[derive(Clone, Copy)]
pub(crate) struct SymbolRelocation {
    pub(crate) target: u64,
    pub(crate) symbol_index: u32,
    pub(crate) symbol_value: u64,
    pub(crate) symbol_name_offset: u32,
    pub(crate) addend: u64,
}

#[derive(Clone, Copy)]
struct DynamicTable<'a> {
    bytes: &'a [u8],
    file_offset: u64,
    virtual_address: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct PlannedSegment<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) file_page: u64,
    pub(crate) virtual_address: u64,
    pub(crate) virtual_page: u64,
    pub(crate) memory_size: usize,
    pub(crate) page_count: usize,
    pub(crate) writable: bool,
    pub(crate) executable: bool,
}

pub(crate) struct LoadPlan<'a> {
    pub(crate) entry: u64,
    pub(crate) program_headers: u64,
    pub(crate) program_header_count: u16,
    pub(crate) executable: PlannedSegment<'a>,
    pub(crate) read_only: Option<PlannedSegment<'a>>,
    pub(crate) writable: PlannedSegment<'a>,
    pub(crate) relro: Option<Relro>,
    pub(crate) tls: Option<TlsTemplate<'a>>,
    pub(crate) dynamic_relocations: Option<DynamicRelocations>,
}

pub(crate) struct InterpretedLoadPlan<'main, 'interpreter> {
    pub(crate) initial_entry: u64,
    pub(crate) main_entry: u64,
    pub(crate) interpreter_base: u64,
    pub(crate) main: LoadPlan<'main>,
    pub(crate) interpreter: LoadPlan<'interpreter>,
}

pub(crate) struct InterpreterAuthority<'a> {
    pub(crate) path: &'a [u8],
    pub(crate) sha256: [u8; 32],
}

#[derive(Clone, Copy)]
pub(crate) struct DependencyAuthority<'a> {
    pub(crate) name: &'a [u8],
    pub(crate) sha256: [u8; 32],
    pub(crate) load_bias: u64,
}

pub(crate) struct DependencyLoadPlan<'main, 'interpreter, 'dependency> {
    pub(crate) process: InterpretedLoadPlan<'main, 'interpreter>,
    pub(crate) dependency_base: u64,
    pub(crate) dependency: LoadPlan<'dependency>,
}

pub(crate) struct DependencyGraphLoadPlan<'main, 'interpreter, 'dependency> {
    pub(crate) process: InterpretedLoadPlan<'main, 'interpreter>,
    pub(crate) dependency_bases: [u64; 2],
    pub(crate) dependencies: [LoadPlan<'dependency>; 2],
}

#[derive(Clone, Copy)]
pub(crate) struct ProcessStartup {
    pub(crate) initial_entry: u64,
    pub(crate) program_headers: u64,
    pub(crate) program_header_count: u16,
    pub(crate) interpreter_base: u64,
    pub(crate) main_entry: u64,
}

pub(crate) const AUXILIARY_VECTOR_WORDS: usize = 36;

pub(crate) fn parse(bytes: &[u8]) -> Option<Image<'_>> {
    if bytes.len() < ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE * MIN_LOAD_SEGMENT_COUNT
        || bytes.get(0..4)? != b"\x7fELF"
        || bytes[4] != 2
        || bytes[5] != 1
        || bytes[6] != 1
        || !matches!(bytes[7], 0 | 3)
        || bytes[8] != 0
        || bytes[9..16].iter().any(|byte| *byte != 0)
        || read_u16(bytes, 18)? != EM_X86_64
        || read_u32(bytes, 20)? != 1
        || read_u64(bytes, 32)? != ELF_HEADER_SIZE as u64
        || read_u32(bytes, 48)? != 0
        || read_u16(bytes, 52)? != ELF_HEADER_SIZE as u16
        || read_u16(bytes, 54)? != PROGRAM_HEADER_SIZE as u16
    {
        return None;
    }
    let kind = match read_u16(bytes, 16)? {
        ET_EXEC => ImageKind::Executable,
        ET_DYN => ImageKind::PositionIndependent,
        _ => return None,
    };
    let program_header_count = usize::from(read_u16(bytes, 56)?);
    if !(MIN_LOAD_SEGMENT_COUNT..=MAX_PROGRAM_HEADER_COUNT).contains(&program_header_count) {
        return None;
    }
    let program_table_end =
        ELF_HEADER_SIZE.checked_add(PROGRAM_HEADER_SIZE.checked_mul(program_header_count)?)?;
    if program_table_end > bytes.len() {
        return None;
    }
    let entry = read_u64(bytes, 24)?;
    let mut loads = [None; MAX_LOAD_SEGMENT_COUNT];
    let mut load_count = 0;
    let mut gnu_stack_seen = false;
    let mut phdr_address = None;
    let mut relro = None;
    let mut interpreter = None;
    let mut dynamic = None;
    let mut tls = None;
    for index in 0..program_header_count {
        let header = ELF_HEADER_SIZE + index * PROGRAM_HEADER_SIZE;
        match read_u32(bytes, header)? {
            PT_LOAD if load_count < MAX_LOAD_SEGMENT_COUNT => {
                loads[load_count] = Some(parse_segment(
                    bytes,
                    header,
                    kind == ImageKind::PositionIndependent,
                )?);
                load_count += 1;
            }
            PT_GNU_STACK if !gnu_stack_seen && valid_gnu_stack(bytes, header) => {
                gnu_stack_seen = true;
            }
            PT_PHDR if phdr_address.is_none() => {
                phdr_address = Some(parse_phdr(bytes, header, program_header_count)?);
            }
            PT_GNU_RELRO if relro.is_none() => {
                relro = Some(parse_relro(bytes, header)?);
            }
            PT_INTERP if interpreter.is_none() => {
                interpreter = Some(parse_interpreter(bytes, header)?);
            }
            PT_DYNAMIC if dynamic.is_none() => {
                dynamic = Some(parse_dynamic_table(bytes, header)?);
            }
            PT_TLS if tls.is_none() => {
                tls = Some(parse_tls(bytes, header)?);
            }
            _ => return None,
        }
    }
    if !(MIN_LOAD_SEGMENT_COUNT..=MAX_LOAD_SEGMENT_COUNT).contains(&load_count) {
        return None;
    }
    let loads = &loads[..load_count];
    let executable = loads
        .iter()
        .flatten()
        .find(|segment| segment.flags & PF_EXECUTE != 0)
        .copied()?;
    let writable = loads
        .iter()
        .flatten()
        .find(|segment| segment.flags & PF_WRITE != 0)
        .copied()?;
    let executable_count = loads
        .iter()
        .flatten()
        .filter(|segment| segment.flags & PF_EXECUTE != 0)
        .count();
    let writable_count = loads
        .iter()
        .flatten()
        .filter(|segment| segment.flags & PF_WRITE != 0)
        .count();
    let read_only = loads
        .iter()
        .flatten()
        .find(|segment| segment.flags == PF_READ)
        .copied();
    if executable_count != 1
        || writable_count != 1
        || usize::from(read_only.is_some()) + 2 != load_count
        || entry < executable.virtual_address
        || entry
            >= executable
                .virtual_address
                .checked_add(executable.memory_size)?
    {
        return None;
    }
    if relro.is_some_and(|range| {
        range.virtual_address < writable.virtual_address
            || range.virtual_address % PAGE_SIZE as u64 != 0
            || range.memory_size > PAGE_SIZE as u64
            || range.virtual_address.checked_add(range.memory_size)
                != writable.virtual_address.checked_add(writable.memory_size)
    }) {
        return None;
    }
    if tls.is_some_and(|template| !tls_within_writable(template, writable)) {
        return None;
    }
    for (index, first) in loads.iter().flatten().enumerate() {
        let first_end = first.virtual_address.checked_add(first.memory_size)?;
        for second in loads[index + 1..].iter().flatten() {
            let second_end = second.virtual_address.checked_add(second.memory_size)?;
            if first.virtual_address < second_end && second.virtual_address < first_end {
                return None;
            }
        }
    }
    let mapped_program_headers = loads.iter().flatten().find_map(|segment| {
        let table_offset = ELF_HEADER_SIZE as u64;
        let table_end = program_table_end as u64;
        (segment.file_offset <= table_offset
            && segment
                .file_offset
                .checked_add(segment.bytes.len() as u64)?
                >= table_end)
            .then(|| {
                segment
                    .virtual_address
                    .checked_add(table_offset - segment.file_offset)
            })?
    });
    let program_headers = match mapped_program_headers {
        Some(address) => address,
        None if kind == ImageKind::PositionIndependent && phdr_address.is_none() => 0,
        None => return None,
    };
    if phdr_address.is_some_and(|address| address != program_headers) {
        return None;
    }
    let dynamic_metadata = match dynamic {
        Some(table) => Some(parse_dynamic_relocations(table, writable, loads)?),
        None => None,
    };
    let dynamic_relocations = dynamic_metadata.map(|metadata| metadata.relocations);
    let mapped_program_table_end = program_headers
        .checked_add(u64::try_from(program_header_count.checked_mul(PROGRAM_HEADER_SIZE)?).ok()?)?;
    if program_headers != 0 && (program_headers..mapped_program_table_end).contains(&entry) {
        return None;
    }
    Some(Image {
        kind,
        entry,
        program_headers,
        program_header_count: program_header_count as u16,
        executable,
        read_only,
        writable,
        relro,
        tls,
        interpreter,
        dynamic_relocations,
        dynamic_symbol: dynamic_metadata.and_then(|metadata| metadata.symbol_name),
        secondary_dynamic_symbol: dynamic_metadata
            .and_then(|metadata| metadata.secondary_symbol_name),
        needed_libraries: dynamic_metadata
            .map(|metadata| metadata.needed_libraries)
            .unwrap_or([None; 2]),
        soname: dynamic_metadata.and_then(|metadata| metadata.soname),
    })
}

pub(crate) fn plan<'a>(image: &Image<'a>) -> Option<LoadPlan<'a>> {
    if image.interpreter.is_some() || image.dynamic_relocations.is_some() {
        return None;
    }
    plan_executable(image)
}

pub(crate) fn verify_unresolved_dynamic_image_rejected() -> bool {
    let bytes = fixture();
    let Some(mut image) = parse(&bytes) else {
        return false;
    };
    image.dynamic_relocations = Some(DynamicRelocations {
        table_address: 0x40_5008,
        relocation_address: 0x40_5100,
        jump_relocation_address: None,
        relocation_count: 1,
        relocation_target: 0x40_5200,
        relocation_addend: 0x40_0100,
        symbol_table_address: None,
        string_table_address: None,
        string_table_size: 0,
        hash_table_address: None,
        hash_is_gnu: false,
        init_function: None,
        preinit_array: None,
        preinit_relocation_target: None,
        preinit_relocation_addend: None,
        init_array: None,
        tls_relocation_target: None,
        symbol_relocation: None,
        jump_slot_relocation: None,
        secondary_symbol_relocation: None,
        secondary_jump_slot_relocation: None,
        symbol_defined: false,
        secondary_symbol_defined: false,
    });
    plan(&image).is_none()
}

fn plan_executable<'a>(image: &Image<'a>) -> Option<LoadPlan<'a>> {
    if image.kind != ImageKind::Executable {
        return None;
    }
    let plan = LoadPlan {
        entry: image.entry,
        program_headers: image.program_headers,
        program_header_count: image.program_header_count,
        executable: plan_segment(image.executable)?,
        read_only: match image.read_only {
            Some(segment) => Some(plan_segment(segment)?),
            None => None,
        },
        writable: plan_segment(image.writable)?,
        relro: image.relro,
        tls: image.tls,
        dynamic_relocations: image.dynamic_relocations,
    };
    (!plan_has_internal_overlap(&plan)).then_some(plan)
}

pub(crate) fn plan_authenticated_dependency<'main, 'interpreter, 'dependency>(
    main: &Image<'main>,
    interpreter_authority: &InterpreterAuthority<'_>,
    interpreter_bytes: &'interpreter [u8],
    interpreter_base: u64,
    dependency_authority: &DependencyAuthority<'_>,
    dependency_bytes: &'dependency [u8],
) -> Option<DependencyLoadPlan<'main, 'interpreter, 'dependency>> {
    plan_authenticated_dependency_at(
        main,
        None,
        interpreter_authority,
        interpreter_bytes,
        interpreter_base,
        dependency_authority,
        dependency_bytes,
    )
}

pub(crate) fn plan_authenticated_pie_dependency_graph<'main, 'interpreter, 'dependency>(
    main: &Image<'main>,
    main_base: u64,
    interpreter_authority: &InterpreterAuthority<'_>,
    interpreter_bytes: &'interpreter [u8],
    interpreter_base: u64,
    dependency_authorities: &[DependencyAuthority<'_>; 2],
    dependency_bytes: [&'dependency [u8]; 2],
) -> Option<DependencyGraphLoadPlan<'main, 'interpreter, 'dependency>> {
    if dependency_authorities[0].name == dependency_authorities[1].name
        || main.interpreter != Some(interpreter_authority.path)
        || main.needed_libraries
            != [
                Some(dependency_authorities[0].name),
                Some(dependency_authorities[1].name),
            ]
        || crate::crypto::sha256(interpreter_bytes) != interpreter_authority.sha256
        || dependency_bytes
            .iter()
            .zip(dependency_authorities)
            .any(|(bytes, authority)| crate::crypto::sha256(bytes) != authority.sha256)
    {
        return None;
    }
    let interpreter = parse(interpreter_bytes)?;
    let dependency_images = [parse(dependency_bytes[0])?, parse(dependency_bytes[1])?];
    let main_dynamic = main.dynamic_relocations?;
    if main.program_headers == 0
        || interpreter.program_headers == 0
        || interpreter.needed_libraries != [None; 2]
        || main.dynamic_symbol.is_none()
        || main.secondary_dynamic_symbol.is_none()
        || main_dynamic.symbol_defined
        || main_dynamic.secondary_symbol_defined
        || main_dynamic.init_function.is_none()
        || main_dynamic.preinit_array != main_dynamic.preinit_relocation_target
        || main_dynamic.preinit_relocation_addend.is_none()
        || main_dynamic.init_array != Some(main_dynamic.relocation_target)
        || main.dynamic_symbol != dependency_images[0].dynamic_symbol
        || main.secondary_dynamic_symbol != dependency_images[1].dynamic_symbol
        || main.dynamic_symbol == main.secondary_dynamic_symbol
    {
        return None;
    }
    for (image, authority) in dependency_images.iter().zip(dependency_authorities) {
        if image.needed_libraries != [None; 2]
            || image.secondary_dynamic_symbol.is_some()
            || image.soname != Some(authority.name)
            || !image.dynamic_relocations.is_some_and(|dynamic| {
                dynamic.symbol_defined
                    && dynamic.init_function.is_some()
                    && dynamic.preinit_array.is_none()
                    && (image.tls.is_some() == dynamic.tls_relocation_target.is_some())
            })
        {
            return None;
        }
    }
    let main_plan = plan_position_independent_at(main, main_base, true)?;
    let interpreter_plan = plan_position_independent(&interpreter, interpreter_base)?;
    let dependency_plans = [
        plan_position_independent(&dependency_images[0], dependency_authorities[0].load_bias)?,
        plan_position_independent(&dependency_images[1], dependency_authorities[1].load_bias)?,
    ];
    let all_plans = [
        &main_plan,
        &interpreter_plan,
        &dependency_plans[0],
        &dependency_plans[1],
    ];
    if all_plans.iter().enumerate().any(|(index, plan)| {
        all_plans[index + 1..]
            .iter()
            .any(|other| plans_overlap(plan, other))
    }) {
        return None;
    }
    Some(DependencyGraphLoadPlan {
        process: InterpretedLoadPlan {
            initial_entry: interpreter_plan.entry,
            main_entry: main_plan.entry,
            interpreter_base,
            main: main_plan,
            interpreter: interpreter_plan,
        },
        dependency_bases: dependency_authorities.map(|authority| authority.load_bias),
        dependencies: dependency_plans,
    })
}

fn plan_authenticated_dependency_at<'main, 'interpreter, 'dependency>(
    main: &Image<'main>,
    main_base: Option<u64>,
    interpreter_authority: &InterpreterAuthority<'_>,
    interpreter_bytes: &'interpreter [u8],
    interpreter_base: u64,
    dependency_authority: &DependencyAuthority<'_>,
    dependency_bytes: &'dependency [u8],
) -> Option<DependencyLoadPlan<'main, 'interpreter, 'dependency>> {
    if main.interpreter != Some(interpreter_authority.path)
        || main.needed_libraries != [Some(dependency_authority.name), None]
        || crate::crypto::sha256(interpreter_bytes) != interpreter_authority.sha256
        || crate::crypto::sha256(dependency_bytes) != dependency_authority.sha256
    {
        return None;
    }
    let interpreter = parse(interpreter_bytes)?;
    let dependency_image = parse(dependency_bytes)?;
    let main_dynamic = main.dynamic_relocations?;
    let dependency_dynamic = dependency_image.dynamic_relocations?;
    if main.program_headers == 0
        || interpreter.program_headers == 0
        || main.dynamic_symbol.is_none()
        || main.secondary_dynamic_symbol.is_some()
        || main.dynamic_symbol != dependency_image.dynamic_symbol
        || main_dynamic.symbol_defined
        || !dependency_dynamic.symbol_defined
        || dependency_image.needed_libraries != [None; 2]
        || dependency_image.soname != Some(dependency_authority.name)
    {
        return None;
    }
    let main_plan = match main_base {
        Some(base) => plan_position_independent_at(main, base, true)?,
        None => plan_executable(main)?,
    };
    let interpreter_plan = plan_position_independent(&interpreter, interpreter_base)?;
    let dependency_plan =
        plan_position_independent(&dependency_image, dependency_authority.load_bias)?;
    if plans_overlap(&main_plan, &interpreter_plan)
        || plans_overlap(&main_plan, &dependency_plan)
        || plans_overlap(&interpreter_plan, &dependency_plan)
    {
        return None;
    }
    Some(DependencyLoadPlan {
        process: InterpretedLoadPlan {
            initial_entry: interpreter_plan.entry,
            main_entry: main_plan.entry,
            interpreter_base,
            main: main_plan,
            interpreter: interpreter_plan,
        },
        dependency_base: dependency_authority.load_bias,
        dependency: dependency_plan,
    })
}

pub(crate) fn static_startup(plan: &LoadPlan<'_>) -> ProcessStartup {
    ProcessStartup {
        initial_entry: plan.entry,
        program_headers: plan.program_headers,
        program_header_count: plan.program_header_count,
        interpreter_base: 0,
        main_entry: plan.entry,
    }
}

pub(crate) fn interpreted_startup(plan: &InterpretedLoadPlan<'_, '_>) -> ProcessStartup {
    ProcessStartup {
        initial_entry: plan.initial_entry,
        program_headers: plan.main.program_headers,
        program_header_count: plan.main.program_header_count,
        interpreter_base: plan.interpreter_base,
        main_entry: plan.main_entry,
    }
}

pub(crate) fn auxiliary_vector(
    startup: ProcessStartup,
    executable_name: u64,
    platform: u64,
) -> [u64; AUXILIARY_VECTOR_WORDS] {
    [
        3,
        startup.program_headers,
        4,
        PROGRAM_HEADER_SIZE as u64,
        5,
        u64::from(startup.program_header_count),
        6,
        PAGE_SIZE as u64,
        7,
        startup.interpreter_base,
        8,
        0,
        11,
        0,
        12,
        0,
        13,
        0,
        14,
        0,
        23,
        0,
        31,
        executable_name,
        9,
        startup.main_entry,
        17,
        100,
        15,
        platform,
        16,
        0,
        26,
        0,
        0,
        0,
    ]
}

fn plans_overlap(first: &LoadPlan<'_>, second: &LoadPlan<'_>) -> bool {
    planned_segments(first).into_iter().flatten().any(|first| {
        planned_segments(second)
            .into_iter()
            .flatten()
            .any(|second| planned_segments_overlap(first, second))
    })
}

fn plan_has_internal_overlap(plan: &LoadPlan<'_>) -> bool {
    let segments = planned_segments(plan);
    segments
        .iter()
        .enumerate()
        .filter_map(|(index, segment)| segment.map(|segment| (index, segment)))
        .any(|(index, first)| {
            segments[index + 1..]
                .iter()
                .flatten()
                .any(|second| planned_segments_overlap(first, *second))
        })
}

fn planned_segments<'a>(plan: &LoadPlan<'a>) -> [Option<PlannedSegment<'a>>; 3] {
    [Some(plan.executable), plan.read_only, Some(plan.writable)]
}

fn planned_segments_overlap(first: PlannedSegment<'_>, second: PlannedSegment<'_>) -> bool {
    let first_end = first
        .virtual_page
        .checked_add(first.page_count as u64 * PAGE_SIZE as u64);
    let second_end = second
        .virtual_page
        .checked_add(second.page_count as u64 * PAGE_SIZE as u64);
    first_end
        .zip(second_end)
        .is_none_or(|(first_end, second_end)| {
            first.virtual_page < second_end && second.virtual_page < first_end
        })
}

pub(crate) fn plan_position_independent<'a>(
    image: &Image<'a>,
    load_bias: u64,
) -> Option<LoadPlan<'a>> {
    plan_position_independent_at(image, load_bias, false)
}

fn plan_position_independent_at<'a>(
    image: &Image<'a>,
    load_bias: u64,
    allow_interpreter: bool,
) -> Option<LoadPlan<'a>> {
    if image.kind != ImageKind::PositionIndependent
        || image.interpreter.is_some() && !allow_interpreter
        || load_bias < MIN_USER_ADDRESS
        || !load_bias.is_multiple_of(PAGE_SIZE as u64)
    {
        return None;
    }
    let executable = relocate_segment(image.executable, load_bias)?;
    let writable = relocate_segment(image.writable, load_bias)?;
    let read_only = match image.read_only {
        Some(segment) => Some(relocate_segment(segment, load_bias)?),
        None => None,
    };
    let relro = match image.relro {
        Some(range) => Some(Relro {
            virtual_address: range.virtual_address.checked_add(load_bias)?,
            memory_size: range.memory_size,
        }),
        None => None,
    };
    let dynamic_relocations = match image.dynamic_relocations {
        Some(dynamic) => Some(DynamicRelocations {
            table_address: dynamic.table_address.checked_add(load_bias)?,
            relocation_address: dynamic.relocation_address.checked_add(load_bias)?,
            jump_relocation_address: match dynamic.jump_relocation_address {
                Some(address) => NonZeroU64::new(address.get().checked_add(load_bias)?),
                None => None,
            },
            relocation_count: dynamic.relocation_count,
            relocation_target: dynamic.relocation_target.checked_add(load_bias)?,
            relocation_addend: dynamic.relocation_addend,
            symbol_table_address: match dynamic.symbol_table_address {
                Some(address) => Some(address.checked_add(load_bias)?),
                None => None,
            },
            string_table_address: match dynamic.string_table_address {
                Some(address) => Some(address.checked_add(load_bias)?),
                None => None,
            },
            string_table_size: dynamic.string_table_size,
            hash_table_address: match dynamic.hash_table_address {
                Some(address) => Some(address.checked_add(load_bias)?),
                None => None,
            },
            hash_is_gnu: dynamic.hash_is_gnu,
            init_function: match dynamic.init_function {
                Some(address) => Some(address.checked_add(load_bias)?),
                None => None,
            },
            preinit_array: match dynamic.preinit_array {
                Some(address) => Some(address.checked_add(load_bias)?),
                None => None,
            },
            preinit_relocation_target: match dynamic.preinit_relocation_target {
                Some(address) => Some(address.checked_add(load_bias)?),
                None => None,
            },
            preinit_relocation_addend: dynamic.preinit_relocation_addend,
            init_array: match dynamic.init_array {
                Some(address) => Some(address.checked_add(load_bias)?),
                None => None,
            },
            tls_relocation_target: match dynamic.tls_relocation_target {
                Some(address) => NonZeroU64::new(address.get().checked_add(load_bias)?),
                None => None,
            },
            symbol_relocation: match dynamic.symbol_relocation {
                Some(relocation) => Some(SymbolRelocation {
                    target: relocation.target.checked_add(load_bias)?,
                    symbol_value: if dynamic.symbol_defined {
                        relocation.symbol_value.checked_add(load_bias)?
                    } else {
                        relocation.symbol_value
                    },
                    ..relocation
                }),
                None => None,
            },
            jump_slot_relocation: match dynamic.jump_slot_relocation {
                Some(relocation) => Some(SymbolRelocation {
                    target: relocation.target.checked_add(load_bias)?,
                    symbol_value: if dynamic.symbol_defined {
                        relocation.symbol_value.checked_add(load_bias)?
                    } else {
                        relocation.symbol_value
                    },
                    ..relocation
                }),
                None => None,
            },
            secondary_symbol_relocation: match dynamic.secondary_symbol_relocation {
                Some(relocation) => Some(SymbolRelocation {
                    target: relocation.target.checked_add(load_bias)?,
                    symbol_value: if dynamic.secondary_symbol_defined {
                        relocation.symbol_value.checked_add(load_bias)?
                    } else {
                        relocation.symbol_value
                    },
                    ..relocation
                }),
                None => None,
            },
            secondary_jump_slot_relocation: match dynamic.secondary_jump_slot_relocation {
                Some(relocation) => Some(SymbolRelocation {
                    target: relocation.target.checked_add(load_bias)?,
                    symbol_value: if dynamic.secondary_symbol_defined {
                        relocation.symbol_value.checked_add(load_bias)?
                    } else {
                        relocation.symbol_value
                    },
                    ..relocation
                }),
                None => None,
            },
            symbol_defined: dynamic.symbol_defined,
            secondary_symbol_defined: dynamic.secondary_symbol_defined,
        }),
        None => None,
    };
    let tls = match image.tls {
        Some(template) => Some(TlsTemplate {
            virtual_address: template.virtual_address.checked_add(load_bias)?,
            ..template
        }),
        None => None,
    };
    let plan = LoadPlan {
        entry: image.entry.checked_add(load_bias)?,
        program_headers: image.program_headers.checked_add(load_bias)?,
        program_header_count: image.program_header_count,
        executable: plan_segment(executable)?,
        read_only: match read_only {
            Some(segment) => Some(plan_segment(segment)?),
            None => None,
        },
        writable: plan_segment(writable)?,
        relro,
        tls,
        dynamic_relocations,
    };
    (!plan_has_internal_overlap(&plan)).then_some(plan)
}

fn relocate_segment(segment: Segment<'_>, load_bias: u64) -> Option<Segment<'_>> {
    let virtual_address = segment.virtual_address.checked_add(load_bias)?;
    let end = virtual_address.checked_add(segment.memory_size)?;
    (virtual_address >= MIN_USER_ADDRESS && end <= MAX_USER_ADDRESS).then_some(Segment {
        virtual_address,
        ..segment
    })
}

fn plan_segment(segment: Segment<'_>) -> Option<PlannedSegment<'_>> {
    let memory_size = usize::try_from(segment.memory_size).ok()?;
    let virtual_page = segment.virtual_address & !(PAGE_SIZE as u64 - 1);
    let page_offset = usize::try_from(segment.virtual_address - virtual_page).ok()?;
    let mapped_size = page_offset.checked_add(memory_size)?;
    let page_count = mapped_size.checked_add(PAGE_SIZE - 1)? / PAGE_SIZE;
    let maximum_pages = if segment.flags & PF_EXECUTE != 0 {
        MAX_EXECUTABLE_SEGMENT_PAGES
    } else {
        MAX_OTHER_SEGMENT_PAGES
    };
    ((1..=maximum_pages).contains(&page_count) && segment.bytes.len() <= memory_size).then_some(
        PlannedSegment {
            bytes: segment.bytes,
            file_page: segment.file_offset & !(PAGE_SIZE as u64 - 1),
            virtual_address: segment.virtual_address,
            virtual_page,
            memory_size,
            page_count,
            writable: segment.flags & PF_WRITE != 0,
            executable: segment.flags & PF_EXECUTE != 0,
        },
    )
}

pub(crate) fn materialize(segment: PlannedSegment<'_>, page: &mut [u8; PAGE_SIZE]) -> bool {
    materialize_page(segment, 0, page)
}

pub(crate) fn planned_file_page_size(
    segment: PlannedSegment<'_>,
    page_index: usize,
) -> Option<usize> {
    if page_index >= segment.page_count {
        return None;
    }
    let page_offset = usize::try_from(segment.virtual_address - segment.virtual_page).ok()?;
    let file_extent = page_offset.checked_add(segment.bytes.len())?;
    let page_start = page_index.checked_mul(PAGE_SIZE)?;
    Some(file_extent.saturating_sub(page_start).min(PAGE_SIZE))
}

pub(crate) fn materialize_page(
    segment: PlannedSegment<'_>,
    page_index: usize,
    page: &mut [u8; PAGE_SIZE],
) -> bool {
    if page_index >= segment.page_count || segment.bytes.len() > segment.memory_size {
        return false;
    }
    page.fill(0);
    let Some(page_start) = segment
        .virtual_page
        .checked_add((page_index * PAGE_SIZE) as u64)
    else {
        return false;
    };
    let Some(file_end) = segment
        .virtual_address
        .checked_add(segment.bytes.len() as u64)
    else {
        return false;
    };
    let copy_start = page_start.max(segment.virtual_address);
    let copy_end = page_start.saturating_add(PAGE_SIZE as u64).min(file_end);
    if copy_start < copy_end {
        let source_start =
            usize::try_from(copy_start - segment.virtual_address).unwrap_or(usize::MAX);
        let destination_start = usize::try_from(copy_start - page_start).unwrap_or(usize::MAX);
        let length = usize::try_from(copy_end - copy_start).unwrap_or(usize::MAX);
        let Some(source_end) = source_start.checked_add(length) else {
            return false;
        };
        let Some(destination_end) = destination_start.checked_add(length) else {
            return false;
        };
        let Some(source) = segment.bytes.get(source_start..source_end) else {
            return false;
        };
        let Some(destination) = page.get_mut(destination_start..destination_end) else {
            return false;
        };
        destination.copy_from_slice(source);
    }
    true
}

pub(crate) fn materialized_page_matches(
    segment: PlannedSegment<'_>,
    page_index: usize,
    page: &[u8; PAGE_SIZE],
) -> bool {
    if page_index >= segment.page_count {
        return false;
    }
    let Some(page_start) = segment
        .virtual_page
        .checked_add((page_index * PAGE_SIZE) as u64)
    else {
        return false;
    };
    page.iter().enumerate().all(|(offset, byte)| {
        let address = page_start + offset as u64;
        let source = address
            .checked_sub(segment.virtual_address)
            .and_then(|offset| {
                usize::try_from(offset)
                    .ok()
                    .and_then(|offset| segment.bytes.get(offset))
            });
        *byte == source.copied().unwrap_or(0)
    })
}

fn parse_segment(bytes: &[u8], header: usize, position_independent: bool) -> Option<Segment<'_>> {
    if read_u32(bytes, header)? != PT_LOAD {
        return None;
    }
    let flags = read_u32(bytes, header + 4)?;
    let offset = usize::try_from(read_u64(bytes, header + 8)?).ok()?;
    let virtual_address = read_u64(bytes, header + 16)?;
    let physical_address = read_u64(bytes, header + 24)?;
    let file_size = usize::try_from(read_u64(bytes, header + 32)?).ok()?;
    let memory_size = read_u64(bytes, header + 40)?;
    let alignment = read_u64(bytes, header + 48)?;
    let end = virtual_address.checked_add(memory_size)?;
    let file_end = offset.checked_add(file_size)?;
    if flags & !(PF_READ | PF_WRITE | PF_EXECUTE) != 0
        || flags & PF_READ == 0
        || flags & (PF_WRITE | PF_EXECUTE) == (PF_WRITE | PF_EXECUTE)
        || physical_address != 0 && physical_address != virtual_address
        || file_size == 0
        || memory_size < file_size as u64
        || !position_independent && virtual_address < MIN_USER_ADDRESS
        || end > MAX_USER_ADDRESS
        || end <= virtual_address
        || file_end > bytes.len()
        || alignment == 0
        || !alignment.is_power_of_two()
        || offset as u64 % alignment != virtual_address % alignment
    {
        return None;
    }
    Some(Segment {
        bytes: bytes.get(offset..file_end)?,
        file_offset: offset as u64,
        virtual_address,
        memory_size,
        flags,
    })
}

fn valid_gnu_stack(bytes: &[u8], header: usize) -> bool {
    read_u32(bytes, header) == Some(PT_GNU_STACK)
        && read_u32(bytes, header + 4) == Some(PF_READ | PF_WRITE)
        && (8..48)
            .step_by(8)
            .all(|offset| read_u64(bytes, header + offset) == Some(0))
        && matches!(read_u64(bytes, header + 48), Some(0 | 16))
}

fn parse_phdr(bytes: &[u8], header: usize, program_header_count: usize) -> Option<u64> {
    let table_size = PROGRAM_HEADER_SIZE.checked_mul(program_header_count)? as u64;
    let virtual_address = read_u64(bytes, header + 16)?;
    let physical_address = read_u64(bytes, header + 24)?;
    (read_u32(bytes, header + 4)? == PF_READ
        && read_u64(bytes, header + 8)? == ELF_HEADER_SIZE as u64
        && (physical_address == 0 || physical_address == virtual_address)
        && read_u64(bytes, header + 32)? == table_size
        && read_u64(bytes, header + 40)? == table_size
        && read_u64(bytes, header + 48)? == 8)
        .then_some(virtual_address)
}

fn parse_relro(bytes: &[u8], header: usize) -> Option<Relro> {
    let file_offset = read_u64(bytes, header + 8)?;
    let virtual_address = read_u64(bytes, header + 16)?;
    let physical_address = read_u64(bytes, header + 24)?;
    let file_size = read_u64(bytes, header + 32)?;
    let memory_size = read_u64(bytes, header + 40)?;
    let alignment = read_u64(bytes, header + 48)?;
    (read_u32(bytes, header + 4)? == PF_READ
        && (physical_address == 0 || physical_address == virtual_address)
        && memory_size != 0
        && file_size <= memory_size
        && alignment != 0
        && alignment.is_power_of_two()
        && file_offset % alignment == virtual_address % alignment)
        .then_some(Relro {
            virtual_address,
            memory_size,
        })
}

fn parse_tls(bytes: &[u8], header: usize) -> Option<TlsTemplate<'_>> {
    let file_offset = read_u64(bytes, header + 8)?;
    let virtual_address = read_u64(bytes, header + 16)?;
    let physical_address = read_u64(bytes, header + 24)?;
    let file_size = read_u64(bytes, header + 32)?;
    let memory_size = read_u64(bytes, header + 40)?;
    let alignment = read_u64(bytes, header + 48)?;
    let file_start = usize::try_from(file_offset).ok()?;
    let file_length = usize::try_from(file_size).ok()?;
    let file_end = file_start.checked_add(file_length)?;
    if read_u32(bytes, header)? != PT_TLS
        || read_u32(bytes, header + 4)? != PF_READ
        || physical_address != 0 && physical_address != virtual_address
        || file_size == 0
        || file_size > memory_size
        || memory_size > MAX_TLS_TEMPLATE_SIZE
        || alignment == 0
        || alignment > MAX_TLS_TEMPLATE_SIZE
        || !alignment.is_power_of_two()
        || file_offset % alignment != virtual_address % alignment
        || file_end > bytes.len()
    {
        return None;
    }
    Some(TlsTemplate {
        bytes: bytes.get(file_start..file_end)?,
        file_offset,
        virtual_address,
        memory_size,
        alignment,
    })
}

fn tls_within_writable(template: TlsTemplate<'_>, writable: Segment<'_>) -> bool {
    let Some(template_memory_end) = template.virtual_address.checked_add(template.memory_size)
    else {
        return false;
    };
    let Some(writable_memory_end) = writable.virtual_address.checked_add(writable.memory_size)
    else {
        return false;
    };
    let Some(template_file_end) = template
        .file_offset
        .checked_add(template.bytes.len() as u64)
    else {
        return false;
    };
    let Some(writable_file_end) = writable
        .file_offset
        .checked_add(writable.bytes.len() as u64)
    else {
        return false;
    };
    template.virtual_address >= writable.virtual_address
        && template_memory_end <= writable_memory_end
        && template.file_offset >= writable.file_offset
        && template_file_end <= writable_file_end
        && template.virtual_address - writable.virtual_address
            == template.file_offset - writable.file_offset
}

fn parse_interpreter(bytes: &[u8], header: usize) -> Option<&[u8]> {
    let virtual_address = read_u64(bytes, header + 16)?;
    let physical_address = read_u64(bytes, header + 24)?;
    if read_u32(bytes, header)? != PT_INTERP
        || read_u32(bytes, header + 4)? != PF_READ
        || physical_address != 0 && physical_address != virtual_address
        || read_u64(bytes, header + 48)? != 1
    {
        return None;
    }
    let offset = usize::try_from(read_u64(bytes, header + 8)?).ok()?;
    let file_size = usize::try_from(read_u64(bytes, header + 32)?).ok()?;
    if !(2..=MAX_INTERPRETER_PATH_SIZE).contains(&file_size)
        || read_u64(bytes, header + 40)? != file_size as u64
    {
        return None;
    }
    let path = bytes.get(offset..offset.checked_add(file_size)?)?;
    (path[0] == b'/' && path[file_size - 1] == 0 && !path[..file_size - 1].contains(&0))
        .then_some(&path[..file_size - 1])
}

fn parse_dynamic_table(bytes: &[u8], header: usize) -> Option<DynamicTable<'_>> {
    let file_offset = read_u64(bytes, header + 8)?;
    let virtual_address = read_u64(bytes, header + 16)?;
    let physical_address = read_u64(bytes, header + 24)?;
    let file_size = usize::try_from(read_u64(bytes, header + 32)?).ok()?;
    let memory_size = usize::try_from(read_u64(bytes, header + 40)?).ok()?;
    let alignment = read_u64(bytes, header + 48)?;
    if read_u32(bytes, header + 4)? != PF_READ | PF_WRITE
        || physical_address != 0 && physical_address != virtual_address
        || file_size != memory_size
        || !(2..=23).contains(&(file_size / DYNAMIC_ENTRY_SIZE))
        || !file_size.is_multiple_of(DYNAMIC_ENTRY_SIZE)
        || alignment != 8
        || file_offset % alignment != virtual_address % alignment
    {
        return None;
    }
    let offset = usize::try_from(file_offset).ok()?;
    Some(DynamicTable {
        bytes: bytes.get(offset..offset.checked_add(file_size)?)?,
        file_offset,
        virtual_address,
    })
}

fn parse_dynamic_relocations<'a>(
    table: DynamicTable<'a>,
    writable: Segment<'a>,
    loads: &[Option<Segment<'a>>],
) -> Option<DynamicMetadata<'a>> {
    let table_offset = table
        .virtual_address
        .checked_sub(writable.virtual_address)?;
    if writable.file_offset.checked_add(table_offset)? != table.file_offset
        || table_offset.checked_add(table.bytes.len() as u64)? > writable.bytes.len() as u64
    {
        return None;
    }
    let mut rela_address = None;
    let mut rela_size = None;
    let mut rela_entry_size = None;
    let mut string_table_address = None;
    let mut string_table_size = None;
    let mut symbol_table_address = None;
    let mut symbol_entry_size = None;
    let mut hash_table_address = None;
    let mut hash_is_gnu = false;
    let mut needed_name_offsets = [None; 2];
    let mut soname_offset = None;
    let mut jump_relocation_address = None;
    let mut jump_relocation_size = None;
    let mut jump_relocation_kind = None;
    let mut plt_got = None;
    let mut debug = None;
    let mut flags = None;
    let mut flags_1 = None;
    let mut relative_count = None;
    let mut init_function = None;
    let mut init_array = None;
    let mut init_array_size = None;
    let mut preinit_array = None;
    let mut preinit_array_size = None;
    let mut terminated = false;
    let entries = table.bytes.as_chunks::<DYNAMIC_ENTRY_SIZE>().0;
    for (index, entry) in entries.iter().enumerate() {
        let tag = read_u64(entry, 0)?;
        let value = read_u64(entry, 8)?;
        match tag {
            DT_NULL if value == 0 && index + 1 == entries.len() => {
                terminated = true;
                break;
            }
            DT_RELA if rela_address.replace(value).is_none() => {}
            DT_RELASZ if rela_size.replace(value).is_none() => {}
            DT_RELAENT if rela_entry_size.replace(value).is_none() => {}
            DT_STRTAB if string_table_address.replace(value).is_none() => {}
            DT_STRSZ if string_table_size.replace(value).is_none() => {}
            DT_SYMTAB if symbol_table_address.replace(value).is_none() => {}
            DT_SYMENT if symbol_entry_size.replace(value).is_none() => {}
            DT_HASH if hash_table_address.replace(value).is_none() => {}
            DT_GNU_HASH if hash_table_address.replace(value).is_none() => {
                hash_is_gnu = true;
            }
            DT_NEEDED => {
                let slot = needed_name_offsets
                    .iter_mut()
                    .find(|offset| offset.is_none())?;
                *slot = Some(value);
            }
            DT_SONAME if soname_offset.replace(value).is_none() => {}
            DT_JMPREL if jump_relocation_address.replace(value).is_none() => {}
            DT_PLTRELSZ if jump_relocation_size.replace(value).is_none() => {}
            DT_PLTREL if jump_relocation_kind.replace(value).is_none() => {}
            DT_PLTGOT if plt_got.replace(value).is_none() => {}
            DT_DEBUG if debug.replace(value).is_none() => {}
            DT_FLAGS if flags.replace(value).is_none() => {}
            DT_FLAGS_1 if flags_1.replace(value).is_none() => {}
            DT_RELACOUNT if relative_count.replace(value).is_none() => {}
            DT_INIT if init_function.replace(value).is_none() => {}
            DT_INIT_ARRAY if init_array.replace(value).is_none() => {}
            DT_INIT_ARRAYSZ if init_array_size.replace(value).is_none() => {}
            DT_PREINIT_ARRAY if preinit_array.replace(value).is_none() => {}
            DT_PREINIT_ARRAYSZ if preinit_array_size.replace(value).is_none() => {}
            _ => return None,
        }
    }
    let (Some(rela_address), Some(rela_size), Some(rela_entry_size)) =
        (rela_address, rela_size, rela_entry_size)
    else {
        return None;
    };
    if !terminated
        || rela_entry_size != RELA_ENTRY_SIZE as u64
        || !matches!(rela_size, value if value == RELA_ENTRY_SIZE as u64
            || value == (2 * RELA_ENTRY_SIZE) as u64
            || value == (3 * RELA_ENTRY_SIZE) as u64
            || value == (4 * RELA_ENTRY_SIZE) as u64)
    {
        return None;
    }
    let writable_end = writable.virtual_address.checked_add(writable.memory_size)?;
    if !optional_dynamic_tags_valid(
        plt_got,
        debug,
        flags_1,
        relative_count,
        writable.virtual_address,
        writable_end,
    ) {
        return None;
    }
    let primary_relocation_count = usize::try_from(rela_size).ok()? / RELA_ENTRY_SIZE;
    let jump_rela = match (
        jump_relocation_address,
        jump_relocation_size,
        jump_relocation_kind,
    ) {
        (None, None, None) => None,
        (Some(address), Some(size), Some(kind))
            if primary_relocation_count == 2
                && size == RELA_ENTRY_SIZE as u64
                && kind == DT_RELA =>
        {
            let primary_end = rela_address.checked_add(rela_size)?;
            let jump_end = address.checked_add(size)?;
            if rela_address < jump_end && address < primary_end {
                return None;
            }
            Some(virtual_file_bytes(loads, address, RELA_ENTRY_SIZE)?)
        }
        (Some(address), Some(size), Some(kind))
            if primary_relocation_count == 3
                && size == RELA_ENTRY_SIZE as u64
                && kind == DT_RELA =>
        {
            let primary_end = rela_address.checked_add(rela_size)?;
            let jump_end = address.checked_add(size)?;
            if rela_address < jump_end && address < primary_end {
                return None;
            }
            Some(virtual_file_bytes(loads, address, RELA_ENTRY_SIZE)?)
        }
        (Some(address), Some(size), Some(kind))
            if primary_relocation_count == 3
                && size == (2 * RELA_ENTRY_SIZE) as u64
                && kind == DT_RELA =>
        {
            let primary_end = rela_address.checked_add(rela_size)?;
            let jump_end = address.checked_add(size)?;
            if rela_address < jump_end && address < primary_end {
                return None;
            }
            Some(virtual_file_bytes(loads, address, 2 * RELA_ENTRY_SIZE)?)
        }
        (Some(address), Some(size), Some(kind))
            if primary_relocation_count == 4
                && size == (2 * RELA_ENTRY_SIZE) as u64
                && kind == DT_RELA =>
        {
            let primary_end = rela_address.checked_add(rela_size)?;
            let jump_end = address.checked_add(size)?;
            if rela_address < jump_end && address < primary_end {
                return None;
            }
            Some(virtual_file_bytes(loads, address, 2 * RELA_ENTRY_SIZE)?)
        }
        _ => return None,
    };
    let jump_relocation_count = jump_rela.map_or(0, |records| records.len() / RELA_ENTRY_SIZE);
    let relocation_count = primary_relocation_count + jump_relocation_count;
    let rela = virtual_file_bytes(
        loads,
        rela_address,
        primary_relocation_count * RELA_ENTRY_SIZE,
    )?;
    let relative_record = |record: &[u8]| -> Option<(u64, u64)> {
        let target = read_u64(record, 0)?;
        let info = read_u64(record, 8)?;
        let addend = read_u64(record, 16)?;
        (info == R_X86_64_RELATIVE
            && target.is_multiple_of(8)
            && target >= writable.virtual_address
            && target.checked_add(8)? <= writable_end)
            .then_some((target, addend))
    };
    let first_relative = relative_record(&rela[..RELA_ENTRY_SIZE])?;
    let second_relative = match (preinit_array, preinit_array_size) {
        (None, None) => None,
        (Some(_), Some(8)) if primary_relocation_count == 4 => Some(relative_record(
            &rela[RELA_ENTRY_SIZE..2 * RELA_ENTRY_SIZE],
        )?),
        _ => return None,
    };
    let (preinit_array, preinit_relocation_target, preinit_relocation_addend, target, addend) =
        match (preinit_array, second_relative) {
            (None, None) => (None, None, None, first_relative.0, first_relative.1),
            (Some(address), Some(second))
                if address.is_multiple_of(8)
                    && address >= writable.virtual_address
                    && address.checked_add(8)? <= writable_end
                    && address == first_relative.0 =>
            {
                (
                    Some(address),
                    Some(first_relative.0),
                    Some(first_relative.1),
                    second.0,
                    second.1,
                )
            }
            _ => return None,
        };
    if relative_count.is_some_and(|count| count != if preinit_array.is_some() { 2 } else { 1 }) {
        return None;
    }
    let executable = loads
        .iter()
        .flatten()
        .find(|segment| segment.flags & PF_EXECUTE != 0)?;
    let executable_end = executable
        .virtual_address
        .checked_add(executable.memory_size)?;
    let executable_addend =
        |value: u64| value >= executable.virtual_address && value < executable_end;
    if preinit_relocation_addend.is_some_and(|value| !executable_addend(value)) {
        return None;
    }
    let init_array = match (init_array, init_array_size) {
        (None, None) => None,
        (Some(address), Some(size))
            if size == 8
                && address.is_multiple_of(8)
                && address == target
                && address >= writable.virtual_address
                && address.checked_add(size)? <= writable_end =>
        {
            if !executable_addend(addend) {
                return None;
            }
            Some(address)
        }
        _ => return None,
    };
    let init_function = match init_function {
        Some(address) => {
            if address == 0 || address < executable.virtual_address || address >= executable_end {
                return None;
            }
            Some(address)
        }
        None => None,
    };
    let symbol_metadata = (
        symbol_table_address,
        string_table_address,
        string_table_size,
        symbol_entry_size,
        hash_table_address,
        soname_offset,
    );
    let tls_relocation_target =
        if primary_relocation_count == 3 && jump_relocation_count == 1 && preinit_array.is_none() {
            Some(parse_tls_relocation(
                &rela[RELA_ENTRY_SIZE..2 * RELA_ENTRY_SIZE],
                writable.virtual_address,
                writable_end,
                target,
            )?)
        } else {
            None
        };
    if flags != tls_relocation_target.map(|_| DF_STATIC_TLS) {
        return None;
    }
    let (
        symbol_table_address,
        string_table_address,
        string_table_size,
        hash_table_address,
        symbol_relocation,
        jump_slot_relocation,
        secondary_symbol_relocation,
        secondary_jump_slot_relocation,
        symbol_name,
        secondary_symbol_name,
        needed_libraries,
        soname,
        symbol_defined,
        secondary_symbol_defined,
    ) = match (relocation_count, symbol_metadata) {
        (1, (None, None, None, None, None, None)) if needed_name_offsets == [None; 2] => (
            None, None, 0, None, None, None, None, None, None, None, [None; 2], None, false, false,
        ),
        (1, (Some(symbols), Some(strings), Some(1), Some(symbol_size), Some(hash), None))
            if symbol_size == SYMBOL_ENTRY_SIZE as u64
                && hash_is_gnu
                && needed_name_offsets == [None; 2] =>
        {
            let symbol = virtual_file_bytes(loads, symbols, SYMBOL_ENTRY_SIZE)?;
            let string = virtual_file_bytes(loads, strings, 1)?;
            if symbol.iter().any(|byte| *byte != 0)
                || string[0] != 0
                || !validate_empty_gnu_hash(loads, hash)?
            {
                return None;
            }
            (
                Some(symbols),
                Some(strings),
                1,
                Some(hash),
                None,
                None,
                None,
                None,
                None,
                None,
                [None; 2],
                None,
                false,
                false,
            )
        }
        (
            count @ 2..=4,
            (
                Some(symbols),
                Some(strings),
                Some(string_size),
                Some(symbol_size),
                Some(hash),
                soname_offset,
            ),
        ) if symbol_size == SYMBOL_ENTRY_SIZE as u64
            && (count == 4) == tls_relocation_target.is_some()
            && (2..=MAX_INTERPRETER_PATH_SIZE as u64).contains(&string_size) =>
        {
            let string_size = usize::try_from(string_size).ok()?;
            let string_table = virtual_file_bytes(loads, strings, string_size)?;
            if string_table[0] != 0 {
                return None;
            }
            let symbol_record = if tls_relocation_target.is_some() {
                2
            } else {
                1
            };
            let second =
                &rela[symbol_record * RELA_ENTRY_SIZE..(symbol_record + 1) * RELA_ENTRY_SIZE];
            let symbol_target = read_u64(second, 0)?;
            let symbol_info = read_u64(second, 8)?;
            let symbol_addend = read_u64(second, 16)?;
            let relocation_kind = symbol_info as u32;
            let symbol_index = u32::try_from(symbol_info >> 32).ok()?;
            if relocation_kind != R_X86_64_GLOB_DAT
                || symbol_index != 1
                || symbol_addend != 0
                || !symbol_target.is_multiple_of(8)
                || symbol_target == target
                || tls_relocation_target.is_some_and(|target| target.get() == symbol_target)
                || symbol_target < writable.virtual_address
                || symbol_target.checked_add(8)? > writable_end
            {
                return None;
            }
            let symbol_bytes = virtual_file_bytes(loads, symbols, 2 * SYMBOL_ENTRY_SIZE)?;
            if symbol_bytes[..SYMBOL_ENTRY_SIZE]
                .iter()
                .any(|byte| *byte != 0)
            {
                return None;
            }
            let symbol = &symbol_bytes[SYMBOL_ENTRY_SIZE..2 * SYMBOL_ENTRY_SIZE];
            let name_offset = read_u32(symbol, 0)?;
            let symbol_value = read_u64(symbol, 8)?;
            let symbol_size = read_u64(symbol, 16)?;
            let name_offset_usize = usize::try_from(name_offset).ok()?;
            let name = string_table.get(name_offset_usize..)?;
            let name_length = name.iter().position(|byte| *byte == 0)?;
            let name = &name[..name_length];
            let parse_name = |offset| -> Option<Option<&[u8]>> {
                match offset {
                    Some(offset) => {
                        let offset = usize::try_from(offset).ok()?;
                        let candidate = string_table.get(offset..)?;
                        let length = candidate.iter().position(|byte| *byte == 0)?;
                        Some((length != 0).then_some(&candidate[..length]))
                    }
                    None => Some(None),
                }
            };
            let needed_libraries = [
                parse_name(needed_name_offsets[0])?,
                parse_name(needed_name_offsets[1])?,
            ];
            if needed_libraries[0].is_some() && needed_libraries[0] == needed_libraries[1] {
                return None;
            }
            let soname = match soname_offset {
                Some(offset) => {
                    let offset = usize::try_from(offset).ok()?;
                    let candidate = string_table.get(offset..)?;
                    let length = candidate.iter().position(|byte| *byte == 0)?;
                    (length != 0).then_some(&candidate[..length])
                }
                None => None,
            };
            let hash_matches = if hash_is_gnu {
                validate_gnu_hash(loads, hash, name, symbol_index)?
            } else {
                validate_sysv_hash(loads, hash, name, symbol_index)?
            };
            let executable = loads
                .iter()
                .flatten()
                .find(|segment| segment.flags & PF_EXECUTE != 0)?;
            let executable_end = executable
                .virtual_address
                .checked_add(executable.memory_size)?;
            let symbol_defined = read_u16(symbol, 6)? != 0;
            if name_offset == 0
                || name.is_empty()
                || !hash_matches
                || symbol[4] != 0x12
                || symbol[5] != 0
                || symbol_defined
                    && (symbol_value < executable.virtual_address
                        || symbol_value >= executable_end
                        || symbol_size > executable_end.checked_sub(symbol_value)?)
                || !symbol_defined
                    && (needed_libraries[0].is_none()
                        || soname.is_some()
                        || symbol_value != 0
                        || symbol_size != 0)
                || symbol_defined && needed_libraries.iter().any(Option::is_some)
            {
                return None;
            }
            let symbol_relocation = SymbolRelocation {
                target: symbol_target,
                symbol_index,
                symbol_value,
                symbol_name_offset: name_offset,
                addend: symbol_addend,
            };
            let jump_slot_relocation = if count >= 3 {
                let third = match jump_rela {
                    Some(rela) => rela,
                    None => &rela[2 * RELA_ENTRY_SIZE..3 * RELA_ENTRY_SIZE],
                };
                let jump_target = read_u64(third, 0)?;
                let jump_info = read_u64(third, 8)?;
                let jump_addend = read_u64(third, 16)?;
                if jump_info as u32 != R_X86_64_JUMP_SLOT
                    || u32::try_from(jump_info >> 32).ok()? != symbol_index
                    || jump_addend != 0
                    || !jump_target.is_multiple_of(8)
                    || jump_target == target
                    || tls_relocation_target.is_some_and(|target| target.get() == jump_target)
                    || jump_target == symbol_target
                    || jump_target < writable.virtual_address
                    || jump_target.checked_add(8)? > writable_end
                {
                    return None;
                }
                Some(SymbolRelocation {
                    target: jump_target,
                    symbol_index,
                    symbol_value,
                    symbol_name_offset: name_offset,
                    addend: jump_addend,
                })
            } else {
                None
            };
            (
                Some(symbols),
                Some(strings),
                string_size,
                Some(hash),
                Some(symbol_relocation),
                jump_slot_relocation,
                None,
                None,
                Some(name),
                None,
                needed_libraries,
                soname,
                symbol_defined,
                false,
            )
        }
        (
            count @ (5 | 6),
            (Some(symbols), Some(strings), Some(string_size), Some(symbol_size), Some(hash), None),
        ) if symbol_size == SYMBOL_ENTRY_SIZE as u64
            && !hash_is_gnu
            && preinit_array.is_some() == (count == 6)
            && (2..=MAX_INTERPRETER_PATH_SIZE as u64).contains(&string_size) =>
        {
            let string_size = usize::try_from(string_size).ok()?;
            let string_table = virtual_file_bytes(loads, strings, string_size)?;
            let symbol_bytes = virtual_file_bytes(loads, symbols, 3 * SYMBOL_ENTRY_SIZE)?;
            let jump_records = jump_rela?;
            if string_table[0] != 0
                || symbol_bytes[..SYMBOL_ENTRY_SIZE]
                    .iter()
                    .any(|byte| *byte != 0)
            {
                return None;
            }
            let parse_string = |offset: u32| -> Option<&[u8]> {
                let offset = usize::try_from(offset).ok()?;
                let candidate = string_table.get(offset..)?;
                let length = candidate.iter().position(|byte| *byte == 0)?;
                (length != 0).then_some(&candidate[..length])
            };
            let parse_needed = |offset: Option<u64>| -> Option<Option<&[u8]>> {
                match offset {
                    Some(offset) => Some(Some(parse_string(u32::try_from(offset).ok()?)?)),
                    None => Some(None),
                }
            };
            let needed_libraries = [
                parse_needed(needed_name_offsets[0])?,
                parse_needed(needed_name_offsets[1])?,
            ];
            if needed_libraries[0].is_none()
                || needed_libraries[1].is_none()
                || needed_libraries[0] == needed_libraries[1]
            {
                return None;
            }

            let first_symbol = &symbol_bytes[SYMBOL_ENTRY_SIZE..2 * SYMBOL_ENTRY_SIZE];
            let second_symbol = &symbol_bytes[2 * SYMBOL_ENTRY_SIZE..3 * SYMBOL_ENTRY_SIZE];
            let first_name_offset = read_u32(first_symbol, 0)?;
            let second_name_offset = read_u32(second_symbol, 0)?;
            let first_name = parse_string(first_name_offset)?;
            let second_name = parse_string(second_name_offset)?;
            if first_name == second_name
                || first_symbol[4] != 0x12
                || second_symbol[4] != 0x12
                || first_symbol[5] != 0
                || second_symbol[5] != 0
                || read_u16(first_symbol, 6)? != 0
                || read_u16(second_symbol, 6)? != 0
                || read_u64(first_symbol, 8)? != 0
                || read_u64(second_symbol, 8)? != 0
                || read_u64(first_symbol, 16)? != 0
                || read_u64(second_symbol, 16)? != 0
                || !validate_sysv_hash_pair(loads, hash, [first_name, second_name])?
            {
                return None;
            }

            let parse_relocation = |record: &[u8], kind: u32, index: u32| {
                let relocation_target = read_u64(record, 0)?;
                let relocation_info = read_u64(record, 8)?;
                let relocation_addend = read_u64(record, 16)?;
                (relocation_info as u32 == kind
                    && u32::try_from(relocation_info >> 32).ok()? == index
                    && relocation_addend == 0
                    && relocation_target.is_multiple_of(8)
                    && relocation_target >= writable.virtual_address
                    && relocation_target.checked_add(8)? <= writable_end)
                    .then_some(relocation_target)
            };
            let symbol_record = if count == 6 { 2 } else { 1 };
            let first_symbol_target = parse_relocation(
                &rela[symbol_record * RELA_ENTRY_SIZE..(symbol_record + 1) * RELA_ENTRY_SIZE],
                R_X86_64_GLOB_DAT,
                1,
            )?;
            let second_symbol_target = parse_relocation(
                &rela[(symbol_record + 1) * RELA_ENTRY_SIZE..(symbol_record + 2) * RELA_ENTRY_SIZE],
                R_X86_64_GLOB_DAT,
                2,
            )?;
            let first_jump_target =
                parse_relocation(&jump_records[..RELA_ENTRY_SIZE], R_X86_64_JUMP_SLOT, 1)?;
            let second_jump_target = parse_relocation(
                &jump_records[RELA_ENTRY_SIZE..2 * RELA_ENTRY_SIZE],
                R_X86_64_JUMP_SLOT,
                2,
            )?;
            let targets = [
                target,
                preinit_relocation_target.unwrap_or(0),
                first_symbol_target,
                second_symbol_target,
                first_jump_target,
                second_jump_target,
            ];
            if targets
                .iter()
                .enumerate()
                .any(|(index, target)| targets[..index].contains(target))
            {
                return None;
            }
            let relocation = |target, symbol_index, symbol_name_offset| SymbolRelocation {
                target,
                symbol_index,
                symbol_value: 0,
                symbol_name_offset,
                addend: 0,
            };
            (
                Some(symbols),
                Some(strings),
                string_size,
                Some(hash),
                Some(relocation(first_symbol_target, 1, first_name_offset)),
                Some(relocation(first_jump_target, 1, first_name_offset)),
                Some(relocation(second_symbol_target, 2, second_name_offset)),
                Some(relocation(second_jump_target, 2, second_name_offset)),
                Some(first_name),
                Some(second_name),
                needed_libraries,
                None,
                false,
                false,
            )
        }
        _ => return None,
    };
    Some(DynamicMetadata {
        relocations: DynamicRelocations {
            table_address: table.virtual_address,
            relocation_address: rela_address,
            jump_relocation_address: match jump_relocation_address {
                Some(address) => NonZeroU64::new(address),
                None => None,
            },
            relocation_count,
            relocation_target: target,
            relocation_addend: addend,
            symbol_table_address,
            string_table_address,
            string_table_size,
            hash_table_address,
            hash_is_gnu,
            init_function,
            preinit_array,
            preinit_relocation_target,
            preinit_relocation_addend,
            init_array,
            tls_relocation_target,
            symbol_relocation,
            jump_slot_relocation,
            secondary_symbol_relocation,
            secondary_jump_slot_relocation,
            symbol_defined,
            secondary_symbol_defined,
        },
        symbol_name,
        secondary_symbol_name,
        needed_libraries,
        soname,
    })
}

fn parse_tls_relocation(
    record: &[u8],
    writable_start: u64,
    writable_end: u64,
    relative_target: u64,
) -> Option<NonZeroU64> {
    if record.len() != RELA_ENTRY_SIZE {
        return None;
    }
    let target = read_u64(record, 0)?;
    let info = read_u64(record, 8)?;
    let addend = read_u64(record, 16)?;
    (info as u32 == R_X86_64_TPOFF64
        && info >> 32 == 0
        && addend == 0
        && target.is_multiple_of(8)
        && target != relative_target
        && target >= writable_start
        && target.checked_add(8)? <= writable_end)
        .then(|| NonZeroU64::new(target))?
}

fn virtual_file_bytes<'a>(
    loads: &[Option<Segment<'a>>],
    address: u64,
    size: usize,
) -> Option<&'a [u8]> {
    loads.iter().flatten().find_map(|segment| {
        let offset = address.checked_sub(segment.virtual_address)?;
        let offset = usize::try_from(offset).ok()?;
        segment.bytes.get(offset..offset.checked_add(size)?)
    })
}

fn elf_hash(name: &[u8]) -> u32 {
    name.iter().fold(0_u32, |hash, byte| {
        let hash = hash.wrapping_shl(4).wrapping_add(u32::from(*byte));
        let high = hash & 0xf000_0000;
        (hash ^ (high >> 24)) & !high
    })
}

fn gnu_hash(name: &[u8]) -> u32 {
    name.iter().fold(5381_u32, |hash, byte| {
        hash.wrapping_mul(33).wrapping_add(u32::from(*byte))
    })
}

fn optional_dynamic_tags_valid(
    plt_got: Option<u64>,
    debug: Option<u64>,
    flags_1: Option<u64>,
    relative_count: Option<u64>,
    writable_start: u64,
    writable_end: u64,
) -> bool {
    !plt_got.is_some_and(|address| {
        !address.is_multiple_of(8)
            || address < writable_start
            || address.checked_add(32).is_none_or(|end| end > writable_end)
    }) && debug.is_none_or(|value| value == 0)
        && flags_1.is_none_or(|value| value == DF_1_PIE)
        && relative_count.is_none_or(|count| matches!(count, 1 | 2))
}

fn validate_sysv_hash(
    loads: &[Option<Segment<'_>>],
    address: u64,
    name: &[u8],
    symbol_index: u32,
) -> Option<bool> {
    let header = virtual_file_bytes(loads, address, 8)?;
    let bucket_count = usize::try_from(read_u32(header, 0)?).ok()?;
    let chain_count = usize::try_from(read_u32(header, 4)?).ok()?;
    if !(1..=2).contains(&bucket_count) || chain_count != 2 {
        return Some(false);
    }
    let table_size = 8_usize.checked_add(bucket_count.checked_add(chain_count)?.checked_mul(4)?)?;
    let table = virtual_file_bytes(loads, address, table_size)?;
    let selected = usize::try_from(elf_hash(name)).ok()? % bucket_count;
    let buckets_valid = (0..bucket_count).all(|index| {
        read_u32(table, 8 + index * 4) == Some(if index == selected { symbol_index } else { 0 })
    });
    let chains_offset = 8 + bucket_count * 4;
    Some(
        buckets_valid
            && read_u32(table, chains_offset)? == 0
            && read_u32(table, chains_offset + 4)? == 0,
    )
}

fn validate_sysv_hash_pair(
    loads: &[Option<Segment<'_>>],
    address: u64,
    names: [&[u8]; 2],
) -> Option<bool> {
    let table = virtual_file_bytes(loads, address, 32)?;
    if read_u32(table, 0)? != 3 || read_u32(table, 4)? != 3 {
        return Some(false);
    }
    let mut expected_buckets = [0_u32; 3];
    for (offset, name) in names.into_iter().enumerate() {
        let bucket = usize::try_from(elf_hash(name)).ok()? % expected_buckets.len();
        if expected_buckets[bucket] != 0 {
            return Some(false);
        }
        expected_buckets[bucket] = u32::try_from(offset + 1).ok()?;
    }
    Some(
        (0..3).all(|index| read_u32(table, 8 + index * 4) == Some(expected_buckets[index]))
            && (0..3).all(|index| read_u32(table, 20 + index * 4) == Some(0)),
    )
}

fn validate_gnu_hash(
    loads: &[Option<Segment<'_>>],
    address: u64,
    name: &[u8],
    symbol_index: u32,
) -> Option<bool> {
    let table = virtual_file_bytes(loads, address, 32)?;
    let hash = gnu_hash(name);
    let bloom_shift = read_u32(table, 12)?;
    if bloom_shift >= 64 {
        return Some(false);
    }
    let bloom = read_u64(table, 16)?;
    let bloom_mask = (1_u64 << (hash % 64)) | (1_u64 << ((hash >> bloom_shift) % 64));
    Some(
        read_u32(table, 0)? == 1
            && read_u32(table, 4)? == symbol_index
            && read_u32(table, 8)? == 1
            && bloom & bloom_mask == bloom_mask
            && read_u32(table, 24)? == symbol_index
            && read_u32(table, 28)? == (hash | 1),
    )
}

fn validate_empty_gnu_hash(loads: &[Option<Segment<'_>>], address: u64) -> Option<bool> {
    let table = virtual_file_bytes(loads, address, 28)?;
    let bloom_shift = read_u32(table, 12)?;
    Some(
        read_u32(table, 0)? == 1
            && read_u32(table, 4)? == 1
            && read_u32(table, 8)? == 1
            && bloom_shift < 64
            && read_u64(table, 16)? == 0
            && read_u32(table, 24)? == 0,
    )
}

pub(crate) fn verify_parser() -> bool {
    let image = fixture();
    let Some(parsed) = parse(&image) else {
        return false;
    };
    if parsed.kind != ImageKind::Executable
        || parsed.entry != 0x40_0100
        || parsed.program_headers != 0x40_0040
        || parsed.program_header_count != 3
        || parsed.executable.bytes.get(0x100..) != Some(PROBE_CODE)
        || parsed.executable.virtual_address != 0x40_0000
        || parsed.executable.memory_size != FIXTURE_EXECUTABLE_MEMORY_SIZE
        || parsed.executable.flags != PF_READ | PF_EXECUTE
        || parsed.read_only.is_some()
        || parsed.interpreter.is_some()
        || parsed.dynamic_relocations.is_some()
        || parsed.writable.bytes != *b"PROBE!!\n"
        || parsed.writable.virtual_address != 0x40_5000
        || parsed.writable.memory_size != (PAGE_SIZE + 512) as u64
        || parsed.writable.flags != PF_READ | PF_WRITE
        || parsed
            .relro
            .is_none_or(|range| range.virtual_address != 0x40_6000 || range.memory_size != 512)
    {
        return false;
    }
    if !verify_metadata_parser(&image)
        || !verify_relro_parser()
        || !verify_interpreter_parser(&image)
    {
        return false;
    }
    let rejections = rejects(&image, 0, b'X')
        && rejects(&image, 4, 1)
        && rejects_u16(&image, 16, 1)
        && rejects_u16(&image, 18, 3)
        && rejects_u64(&image, 24, 0x50_0000)
        && rejects_u32(&image, ELF_HEADER_SIZE + 4, PF_READ | PF_WRITE | PF_EXECUTE)
        && rejects_u64(&image, ELF_HEADER_SIZE + 24, 1)
        && rejects_u64(&image, ELF_HEADER_SIZE + 32, u64::MAX)
        && rejects_u64(
            &image,
            ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE + 16,
            0x40_0000,
        )
        && rejects_u32(&image, ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE, 3)
        && rejects_u32(
            &image,
            ELF_HEADER_SIZE + 2 * PROGRAM_HEADER_SIZE + 4,
            PF_READ | PF_WRITE | PF_EXECUTE,
        )
        && rejects_entry_in_program_headers(&image)
        && rejects_u16(&image, 56, (MAX_PROGRAM_HEADER_COUNT + 1) as u16)
        && parse(&image[..image.len() - 1]).is_none();
    rejections
}

fn rejects_entry_in_program_headers(image: &[u8; FIXTURE_SIZE]) -> bool {
    let mut candidate = *image;
    write_u16(&mut candidate, 56, 4);
    write_gnu_stack(&mut candidate, ELF_HEADER_SIZE + 3 * PROGRAM_HEADER_SIZE);
    parse(&candidate).is_none()
}

#[inline(never)]
fn verify_metadata_parser(image: &[u8; FIXTURE_SIZE]) -> bool {
    let stack_header = ELF_HEADER_SIZE + 2 * PROGRAM_HEADER_SIZE;
    let extended_phdr_header = ELF_HEADER_SIZE + 4 * PROGRAM_HEADER_SIZE;
    let mut candidate = *image;

    write_u16(&mut candidate, 56, MIN_LOAD_SEGMENT_COUNT as u16);
    if parse(&candidate).is_none() {
        return false;
    }

    candidate = *image;
    write_phdr(&mut candidate, stack_header, 3);
    if parse(&candidate).is_none_or(|image| image.program_headers != 0x40_0040) {
        return false;
    }

    candidate = *image;
    configure_extended_metadata(&mut candidate);
    if parse(&candidate).is_none() {
        return false;
    }

    swap_program_headers(&mut candidate, ELF_HEADER_SIZE, extended_phdr_header);
    if parse(&candidate).is_none_or(|image| {
        image.executable.flags != PF_READ | PF_EXECUTE || image.writable.flags != PF_READ | PF_WRITE
    }) {
        return false;
    }

    candidate = *image;
    configure_extended_metadata(&mut candidate);
    write_gnu_stack(&mut candidate, extended_phdr_header);
    if parse(&candidate).is_some() {
        return false;
    }

    candidate = *image;
    configure_extended_metadata(&mut candidate);
    write_phdr(&mut candidate, stack_header, 5);
    if parse(&candidate).is_some() {
        return false;
    }

    candidate = *image;
    write_phdr(&mut candidate, stack_header, 3);
    write_u64(&mut candidate, stack_header + 16, 0x40_0048);
    if parse(&candidate).is_some() {
        return false;
    }

    candidate = *image;
    write_phdr(&mut candidate, stack_header, 3);
    write_u64(&mut candidate, stack_header + 32, 1);
    if parse(&candidate).is_some() {
        return false;
    }

    candidate = *image;
    configure_extended_metadata(&mut candidate);
    write_u64(&mut candidate, ELF_HEADER_SIZE + 24, 0x40_0000);
    write_u64(
        &mut candidate,
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE + 24,
        0x40_5000,
    );
    write_u64(&mut candidate, extended_phdr_header + 24, 0x40_0040);
    if parse(&candidate).is_none() {
        return false;
    }

    candidate = *image;
    write_gnu_stack(&mut candidate, stack_header);
    write_u64(&mut candidate, stack_header + 48, 0);
    parse(&candidate).is_some()
}

fn configure_extended_metadata(bytes: &mut [u8]) {
    let phdr_header = ELF_HEADER_SIZE + 3 * PROGRAM_HEADER_SIZE;
    let extended_phdr_header = ELF_HEADER_SIZE + 4 * PROGRAM_HEADER_SIZE;
    write_u16(bytes, 56, 5);
    write_u64(bytes, 24, 0x40_0200);
    write_gnu_stack(bytes, phdr_header);
    write_phdr(bytes, extended_phdr_header, 5);
}

#[inline(never)]
fn verify_interpreter_parser(image: &[u8; FIXTURE_SIZE]) -> bool {
    const PATH: &[u8] = b"/lib64/ld-linux-x86-64.so.2\0";
    const PATH_OFFSET: usize = 0x3f80;
    const SECOND_PATH_OFFSET: usize = 0x3fc0;
    let header = ELF_HEADER_SIZE + 3 * PROGRAM_HEADER_SIZE;
    let duplicate_header = ELF_HEADER_SIZE + 4 * PROGRAM_HEADER_SIZE;
    let mut candidate = *image;
    write_u16(&mut candidate, 56, 4);
    write_u64(&mut candidate, 24, 0x40_0120);
    write_interpreter(&mut candidate, header, PATH_OFFSET, PATH);
    let valid = parse(&candidate).is_some_and(|image| {
        image.interpreter == Some(&PATH[..PATH.len() - 1]) && plan(&image).is_none()
    });

    candidate = *image;
    write_u16(&mut candidate, 56, 5);
    write_interpreter(&mut candidate, header, PATH_OFFSET, PATH);
    write_interpreter(&mut candidate, duplicate_header, SECOND_PATH_OFFSET, PATH);
    let duplicate_rejected = parse(&candidate).is_none();

    candidate = *image;
    write_u16(&mut candidate, 56, 4);
    write_interpreter(&mut candidate, header, PATH_OFFSET, PATH);
    candidate[PATH_OFFSET + PATH.len() - 1] = b'X';
    let unterminated_rejected = parse(&candidate).is_none();

    candidate = *image;
    write_u16(&mut candidate, 56, 4);
    write_interpreter(&mut candidate, header, PATH_OFFSET, PATH);
    candidate[PATH_OFFSET] = b'l';
    let relative_rejected = parse(&candidate).is_none();

    candidate = *image;
    write_u16(&mut candidate, 56, 4);
    write_interpreter(&mut candidate, header, PATH_OFFSET, PATH);
    write_u32(&mut candidate, header + 4, PF_READ | PF_WRITE);
    valid
        && duplicate_rejected
        && unterminated_rejected
        && relative_rejected
        && parse(&candidate).is_none()
}

#[inline(never)]
fn verify_relro_parser() -> bool {
    let stack_header = ELF_HEADER_SIZE + 2 * PROGRAM_HEADER_SIZE;
    let phdr_header = ELF_HEADER_SIZE + 3 * PROGRAM_HEADER_SIZE;
    let mut candidate = fixture();
    write_u16(&mut candidate, 56, 3);
    write_relro(&mut candidate, stack_header, 0x40_6000, 512);
    let valid = parse(&candidate).is_some_and(|image| {
        image
            .relro
            .is_some_and(|range| range.virtual_address == 0x40_6000 && range.memory_size == 512)
    });
    candidate = fixture();
    write_u16(&mut candidate, 56, 3);
    write_relro(&mut candidate, stack_header, 0x40_6000, 512);
    write_u16(&mut candidate, 56, 4);
    write_relro(&mut candidate, phdr_header, 0x40_6000, 512);
    let duplicate_rejected = parse(&candidate).is_none();
    candidate = fixture();
    write_u16(&mut candidate, 56, 3);
    write_relro(&mut candidate, stack_header, 0x40_5001, 512);
    let unaligned_rejected = parse(&candidate).is_none();
    candidate = fixture();
    write_u16(&mut candidate, 56, 3);
    write_relro(
        &mut candidate,
        stack_header,
        0x40_6000,
        PAGE_SIZE as u64 + 1,
    );
    let oversized_rejected = parse(&candidate).is_none();
    candidate = fixture();
    write_u16(&mut candidate, 56, 3);
    write_relro(&mut candidate, stack_header, 0x40_5000, 512);
    valid
        && duplicate_rejected
        && unaligned_rejected
        && oversized_rejected
        && parse(&candidate).is_none()
}

pub(crate) fn verify_load_plan() -> bool {
    let image = fixture();
    let Some(parsed) = parse(&image) else {
        crate::serial_write_bytes(b"\r\nginux: static load fixture parse failed");
        return false;
    };
    let Some(current_plan) = plan(&parsed) else {
        crate::serial_write_bytes(b"\r\nginux: static load fixture planning failed");
        return false;
    };
    if current_plan.entry != 0x40_0100 {
        crate::serial_write_bytes(b"\r\nginux: static load entry invariant failed");
        return false;
    }
    if current_plan.program_headers != 0x40_0040
        || current_plan.program_header_count != 3
        || current_plan.executable.virtual_page != 0x40_0000
        || current_plan.executable.virtual_address != 0x40_0000
        || current_plan.executable.page_count != 5
        || !current_plan.executable.executable
        || current_plan.executable.writable
        || current_plan.read_only.is_some()
        || !current_plan
            .relro
            .is_some_and(|range| range.virtual_address == 0x40_6000 && range.memory_size == 512)
        || current_plan.writable.virtual_page != 0x40_5000
        || current_plan.writable.virtual_address != 0x40_5000
        || current_plan.writable.page_count != 2
        || !current_plan.writable.writable
        || current_plan.writable.executable
    {
        crate::serial_write_bytes(b"\r\nginux: static load metadata invariant failed");
        return false;
    }

    let mut page = [0xa5_u8; PAGE_SIZE];
    let first_page_code = PAGE_SIZE - 0x100;
    let second_page_code = (PROBE_CODE.len() - first_page_code).min(PAGE_SIZE);
    let third_page_offset = first_page_code + second_page_code;
    let third_page_code = (PROBE_CODE.len() - third_page_offset).min(PAGE_SIZE);
    let fourth_page_offset = third_page_offset + third_page_code;
    let fourth_page_code = (PROBE_CODE.len() - fourth_page_offset).min(PAGE_SIZE);
    let fifth_page_offset = fourth_page_offset + fourth_page_code;
    let fifth_page_code = PROBE_CODE.len() - fifth_page_offset;
    if !materialize(current_plan.executable, &mut page)
        || page[..4] != *b"\x7fELF"
        || page[0x100..] != PROBE_CODE[..first_page_code]
        || !materialize_page(current_plan.executable, 1, &mut page)
        || page[..second_page_code]
            != PROBE_CODE[first_page_code..first_page_code + second_page_code]
        || page[second_page_code..].iter().any(|byte| *byte != 0)
        || !materialize_page(current_plan.executable, 2, &mut page)
        || page[..third_page_code]
            != PROBE_CODE[third_page_offset..third_page_offset + third_page_code]
        || page[third_page_code..].iter().any(|byte| *byte != 0)
        || !materialize_page(current_plan.executable, 3, &mut page)
        || page[..fourth_page_code]
            != PROBE_CODE[fourth_page_offset..fourth_page_offset + fourth_page_code]
        || page[fourth_page_code..].iter().any(|byte| *byte != 0)
        || !materialize_page(current_plan.executable, 4, &mut page)
        || page[..fifth_page_code] != PROBE_CODE[fifth_page_offset..]
        || page[fifth_page_code..].iter().any(|byte| *byte != 0)
        || !materialize(current_plan.writable, &mut page)
        || page[..8] != *b"PROBE!!\n"
        || page[8..].iter().any(|byte| *byte != 0)
        || !materialize_page(current_plan.writable, 1, &mut page)
        || page.iter().any(|byte| *byte != 0)
    {
        crate::serial_write_bytes(b"\r\nginux: static load materialization invariant failed");
        return false;
    }
    if !verify_oversized_writable_plan_rejected() {
        crate::serial_write_bytes(b"\r\nginux: oversized load rejection invariant failed");
        return false;
    }
    if !verify_page_aliasing_plan_rejected() {
        crate::serial_write_bytes(b"\r\nginux: aliased load rejection invariant failed");
        return false;
    }
    if !verify_position_independent_plan() {
        crate::serial_write_bytes(b"\r\nginux: position-independent load invariant failed");
        return false;
    }
    true
}

#[inline(never)]
fn verify_position_independent_plan() -> bool {
    let mut image = fixture();
    write_u16(&mut image, 16, ET_DYN);
    write_u64(&mut image, 24, 0x100);
    write_u64(&mut image, ELF_HEADER_SIZE + 16, 0);
    write_u64(
        &mut image,
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE + 16,
        0x5000,
    );
    let relro_header = ELF_HEADER_SIZE + 2 * PROGRAM_HEADER_SIZE;
    write_u64(&mut image, relro_header + 16, 0x6000);
    write_u64(&mut image, relro_header + 24, 0x6000);
    let Some(parsed) = parse(&image) else {
        return false;
    };
    let Some(relocated) = plan_position_independent(&parsed, 0x60_0000) else {
        return false;
    };
    parsed.kind == ImageKind::PositionIndependent
        && parsed.program_headers == 0x40
        && plan(&parsed).is_none()
        && relocated.entry == 0x60_0100
        && relocated.program_headers == 0x60_0040
        && relocated.executable.virtual_address == 0x60_0000
        && relocated.writable.virtual_address == 0x60_5000
        && relocated
            .relro
            .is_some_and(|range| range.virtual_address == 0x60_6000)
        && plan_position_independent(&parsed, 0).is_none()
        && plan_position_independent(&parsed, 0x60_0001).is_none()
        && plan_position_independent(&parsed, MAX_USER_ADDRESS).is_none()
}

#[inline(never)]
pub(crate) fn verify_interpreted_plan() -> bool {
    const PATH: &[u8] = b"/lib64/ld-ginux.so";
    const DEPENDENCY: &[u8] = b"libginux.so";
    let main_bytes = interpreted_main_fixture();
    let interpreter_bytes = compact_position_independent_fixture();
    let dependency_bytes = compact_dependency_fixture();
    let Some(main) = parse(&main_bytes) else {
        crate::serial_write_bytes(b"\r\nginux: interpreted main fixture parse failed");
        return false;
    };
    let authority = InterpreterAuthority {
        path: PATH,
        sha256: crate::crypto::sha256(&interpreter_bytes),
    };
    let dependency_authority = DependencyAuthority {
        name: DEPENDENCY,
        sha256: crate::crypto::sha256(&dependency_bytes),
        load_bias: 0x61_0000,
    };
    if parse(&dependency_bytes).is_none() {
        crate::serial_write_bytes(b"\r\nginux: dependency fixture parse failed");
        return false;
    }
    let Some(combined_plan) = plan_authenticated_dependency(
        &main,
        &authority,
        &interpreter_bytes,
        0x60_0000,
        &dependency_authority,
        &dependency_bytes,
    ) else {
        crate::serial_write_bytes(b"\r\nginux: authenticated dependency plan failed");
        return false;
    };
    let combined = &combined_plan.process;
    let startup = interpreted_startup(combined);
    let auxiliary = auxiliary_vector(startup, 0x70_1000, 0x70_2000);
    if combined_plan.dependency.entry != 0x61_0100 {
        crate::serial_write_bytes(b"\r\nginux: dependency entry plan mismatch");
    }
    if !combined_plan
        .dependency
        .dynamic_relocations
        .is_some_and(|dynamic| {
            dynamic.symbol_defined
                && dynamic
                    .symbol_relocation
                    .is_some_and(|relocation| relocation.symbol_value == 0x61_0100)
        })
    {
        crate::serial_write_bytes(b"\r\nginux: dependency symbol plan mismatch");
    }
    let startup_valid = startup.initial_entry == 0x60_0100
        && startup.program_headers == 0x3f_f040
        && startup.program_header_count == 6
        && startup.interpreter_base == 0x60_0000
        && startup.main_entry == 0x40_0100
        && auxiliary[1] == 0x3f_f040
        && auxiliary[5] == 6
        && auxiliary[9] == 0x60_0000
        && auxiliary[23] == 0x70_1000
        && auxiliary[25] == 0x40_0100
        && auxiliary[29] == 0x70_2000
        && auxiliary[34..] == [0, 0];
    let main_valid = combined.initial_entry == 0x60_0100
        && combined.main_entry == 0x40_0100
        && combined.interpreter_base == 0x60_0000
        && combined.main.program_headers == 0x3f_f040
        && combined.main.program_header_count == 6
        && combined.main.dynamic_relocations.is_some_and(|dynamic| {
            dynamic.table_address == 0x40_5008
                && dynamic.relocation_address
                    == 0x3f_f000 + (ELF_HEADER_SIZE + 6 * PROGRAM_HEADER_SIZE) as u64
                && dynamic.jump_relocation_address.map(NonZeroU64::get)
                    == Some(
                        0x3f_f000
                            + (ELF_HEADER_SIZE + 6 * PROGRAM_HEADER_SIZE + 2 * RELA_ENTRY_SIZE)
                                as u64,
                    )
                && dynamic.relocation_count == 3
                && dynamic.relocation_target == 0x40_6100
                && dynamic.relocation_addend == 0x40_0100
                && dynamic.symbol_table_address == Some(0x40_50d8)
                && dynamic.string_table_address == Some(0x40_5108)
                && dynamic.string_table_size == 25
                && dynamic.hash_table_address == Some(0x40_5124)
                && !dynamic.hash_is_gnu
                && !dynamic.symbol_defined
                && dynamic.symbol_relocation.is_some_and(|relocation| {
                    relocation.target == 0x40_6108
                        && relocation.symbol_index == 1
                        && relocation.symbol_value == 0
                        && relocation.symbol_name_offset == 1
                        && relocation.addend == 0
                })
                && dynamic.jump_slot_relocation.is_some_and(|relocation| {
                    relocation.target == 0x40_6110
                        && relocation.symbol_index == 1
                        && relocation.symbol_value == 0
                        && relocation.symbol_name_offset == 1
                        && relocation.addend == 0
                })
        })
        && combined
            .main
            .read_only
            .is_some_and(|segment| segment.virtual_address == 0x3f_f000);
    let interpreter_valid = combined.interpreter.program_headers == 0x60_0040
        && combined
            .interpreter
            .dynamic_relocations
            .is_some_and(|dynamic| {
                dynamic.table_address == 0x60_3008
                    && dynamic.relocation_address == 0x60_3048
                    && dynamic.relocation_count == 1
                    && dynamic.relocation_target == 0x60_3080
                    && dynamic.relocation_addend == 0x1234
                    && dynamic.symbol_table_address.is_none()
                    && dynamic.string_table_address.is_none()
                    && dynamic.string_table_size == 0
                    && dynamic.hash_table_address.is_none()
                    && !dynamic.hash_is_gnu
                    && dynamic.symbol_relocation.is_none()
                    && dynamic.jump_slot_relocation.is_none()
                    && !dynamic.symbol_defined
            })
        && combined.interpreter.executable.virtual_page == 0x60_0000
        && combined.interpreter.writable.virtual_page == 0x60_3000;
    let dependency_valid = combined_plan.dependency_base == 0x61_0000
        && combined_plan.dependency.entry == 0x61_0100
        && combined_plan.dependency.executable.virtual_page == 0x61_0000
        && combined_plan.dependency.writable.virtual_page == 0x61_3000
        && combined_plan
            .dependency
            .dynamic_relocations
            .is_some_and(|dynamic| {
                dynamic.symbol_defined
                    && dynamic.hash_is_gnu
                    && dynamic
                        .symbol_relocation
                        .is_some_and(|relocation| relocation.symbol_value == 0x61_0100)
            });
    let rejections_valid = verify_dependency_authentication_rejections(
        &main,
        &interpreter_bytes,
        &authority,
        &dependency_bytes,
        &dependency_authority,
    );
    if !startup_valid {
        crate::serial_write_bytes(b"\r\nginux: dependency startup invariant failed");
    }
    if !main_valid {
        crate::serial_write_bytes(b"\r\nginux: dependency main invariant failed");
    }
    if !interpreter_valid {
        crate::serial_write_bytes(b"\r\nginux: dependency interpreter invariant failed");
    }
    if !dependency_valid {
        crate::serial_write_bytes(b"\r\nginux: dependency image invariant failed");
    }
    if !rejections_valid {
        crate::serial_write_bytes(b"\r\nginux: dependency rejection invariant failed");
    }
    startup_valid && main_valid && interpreter_valid && dependency_valid && rejections_valid
}

fn verify_dependency_authentication_rejections(
    main: &Image<'_>,
    interpreter_bytes: &[u8],
    authority: &InterpreterAuthority<'_>,
    dependency_bytes: &[u8],
    dependency_authority: &DependencyAuthority<'_>,
) -> bool {
    let wrong_path = InterpreterAuthority {
        path: b"/lib64/other.so",
        sha256: authority.sha256,
    };
    let mut wrong_digest = authority.sha256;
    wrong_digest[0] ^= 1;
    let wrong_digest = InterpreterAuthority {
        path: authority.path,
        sha256: wrong_digest,
    };
    let wrong_dependency_name = DependencyAuthority {
        name: b"other.so",
        ..*dependency_authority
    };
    let mut dependency_digest = dependency_authority.sha256;
    dependency_digest[0] ^= 1;
    let wrong_dependency_digest = DependencyAuthority {
        sha256: dependency_digest,
        ..*dependency_authority
    };
    let mut mismatched_symbol_dependency = [0_u8; 1024];
    if dependency_bytes.len() != mismatched_symbol_dependency.len() {
        return false;
    }
    mismatched_symbol_dependency.copy_from_slice(dependency_bytes);
    let Some(symbol_name) = mismatched_symbol_dependency
        .windows(b"ginux_probe".len())
        .position(|candidate| candidate == b"ginux_probe")
    else {
        return false;
    };
    mismatched_symbol_dependency[symbol_name] = b'x';
    let mismatched_symbol_authority = DependencyAuthority {
        sha256: crate::crypto::sha256(&mismatched_symbol_dependency),
        ..*dependency_authority
    };
    let mut mismatched_soname_dependency = [0_u8; 1024];
    mismatched_soname_dependency.copy_from_slice(dependency_bytes);
    let Some(soname) = mismatched_soname_dependency
        .windows(b"libginux.so".len())
        .position(|candidate| candidate == b"libginux.so")
    else {
        return false;
    };
    mismatched_soname_dependency[soname] = b'x';
    let mismatched_soname_authority = DependencyAuthority {
        sha256: crate::crypto::sha256(&mismatched_soname_dependency),
        ..*dependency_authority
    };
    plan_authenticated_dependency(
        main,
        &wrong_path,
        interpreter_bytes,
        0x60_0000,
        dependency_authority,
        dependency_bytes,
    )
    .is_none()
        && plan_authenticated_dependency(
            main,
            &wrong_digest,
            interpreter_bytes,
            0x60_0000,
            dependency_authority,
            dependency_bytes,
        )
        .is_none()
        && plan_authenticated_dependency(
            main,
            authority,
            interpreter_bytes,
            0x60_0000,
            &wrong_dependency_name,
            dependency_bytes,
        )
        .is_none()
        && plan_authenticated_dependency(
            main,
            authority,
            interpreter_bytes,
            0x60_0000,
            &wrong_dependency_digest,
            dependency_bytes,
        )
        .is_none()
        && plan_authenticated_dependency(
            main,
            authority,
            interpreter_bytes,
            0x60_0000,
            dependency_authority,
            &dependency_bytes[..dependency_bytes.len() - 1],
        )
        .is_none()
        && plan_authenticated_dependency(
            main,
            authority,
            interpreter_bytes,
            0x60_0000,
            &DependencyAuthority {
                load_bias: 0x60_0000,
                ..*dependency_authority
            },
            dependency_bytes,
        )
        .is_none()
        && plan_authenticated_dependency(
            main,
            authority,
            interpreter_bytes,
            0x60_0000,
            dependency_authority,
            &mismatched_symbol_dependency,
        )
        .is_none()
        && plan_authenticated_dependency(
            main,
            authority,
            interpreter_bytes,
            0x60_0000,
            &mismatched_symbol_authority,
            &mismatched_symbol_dependency,
        )
        .is_none()
        && plan_authenticated_dependency(
            main,
            authority,
            interpreter_bytes,
            0x60_0000,
            &mismatched_soname_authority,
            &mismatched_soname_dependency,
        )
        .is_none()
}

pub(crate) fn verify_dynamic_relocation_rejections() -> bool {
    (0..32).all(dynamic_relocation_rejected)
        && (0..7).all(gnu_hash_rejected)
        && (0..6).all(empty_gnu_hash_rejected)
        && verify_optional_dynamic_tag_rejections()
}

fn verify_optional_dynamic_tag_rejections() -> bool {
    let start = 0x5000;
    let end = 0x6400;
    optional_dynamic_tags_valid(Some(0x6300), Some(0), Some(DF_1_PIE), Some(1), start, end)
        && optional_dynamic_tags_valid(None, None, None, Some(2), start, end)
        && !optional_dynamic_tags_valid(Some(0x6301), None, None, None, start, end)
        && !optional_dynamic_tags_valid(Some(0x63e8), None, None, None, start, end)
        && !optional_dynamic_tags_valid(None, Some(1), None, None, start, end)
        && !optional_dynamic_tags_valid(None, None, Some(0), None, start, end)
        && !optional_dynamic_tags_valid(None, None, None, Some(3), start, end)
}

#[inline(never)]
fn gnu_hash_rejected(case: u8) -> bool {
    const DATA_OFFSET: usize = 0x200;
    const DYNAMIC_OFFSET: usize = DATA_OFFSET + 8;
    const SYMTAB_OFFSET: usize = DYNAMIC_OFFSET + 13 * DYNAMIC_ENTRY_SIZE;
    const STRTAB_OFFSET: usize = SYMTAB_OFFSET + 2 * SYMBOL_ENTRY_SIZE;
    const HASH_OFFSET: usize = (STRTAB_OFFSET + 25 + 3) & !3;
    let mut image = compact_dependency_fixture();
    match case {
        0 => write_u32(&mut image, HASH_OFFSET, 0),
        1 => write_u32(&mut image, HASH_OFFSET + 4, 0),
        2 => write_u32(&mut image, HASH_OFFSET + 8, 0),
        3 => write_u32(&mut image, HASH_OFFSET + 12, 64),
        4 => write_u64(&mut image, HASH_OFFSET + 16, 0),
        5 => write_u32(&mut image, HASH_OFFSET + 24, 0),
        6 => write_u32(&mut image, HASH_OFFSET + 28, 0),
        _ => return false,
    }
    parse(&image).is_none()
}

#[inline(never)]
fn empty_gnu_hash_rejected(case: u8) -> bool {
    let mut table = [0_u8; 28];
    write_u32(&mut table, 0, 1);
    write_u32(&mut table, 4, 1);
    write_u32(&mut table, 8, 1);
    write_u32(&mut table, 12, 26);
    match case {
        0 => write_u32(&mut table, 0, 0),
        1 => write_u32(&mut table, 4, 0),
        2 => write_u32(&mut table, 8, 0),
        3 => write_u32(&mut table, 12, 64),
        4 => write_u64(&mut table, 16, 1),
        5 => write_u32(&mut table, 24, 1),
        _ => return false,
    }
    let loads = [
        Some(Segment {
            bytes: &table,
            file_offset: 0,
            virtual_address: 0,
            memory_size: table.len() as u64,
            flags: PF_READ,
        }),
        None,
        None,
    ];
    validate_empty_gnu_hash(&loads, 0) == Some(false)
}

#[inline(never)]
fn dynamic_relocation_rejected(case: u8) -> bool {
    const RELA_OFFSET: usize = ELF_HEADER_SIZE + 6 * PROGRAM_HEADER_SIZE;
    const DYNAMIC_OFFSET: usize = INTERPRETED_DATA_OFFSET + 8;
    const SYMTAB_OFFSET: usize = DYNAMIC_OFFSET + 13 * DYNAMIC_ENTRY_SIZE;
    const STRTAB_OFFSET: usize = SYMTAB_OFFSET + 2 * SYMBOL_ENTRY_SIZE;
    const HASH_OFFSET: usize = (STRTAB_OFFSET + 25 + 3) & !3;
    let mut image = interpreted_main_fixture();
    match case {
        0 => write_u64(&mut image, DYNAMIC_OFFSET, DT_RELA),
        1 => write_u64(&mut image, DYNAMIC_OFFSET + 3 * DYNAMIC_ENTRY_SIZE + 8, 16),
        2 => write_u64(&mut image, RELA_OFFSET + 8, 1),
        3 => write_u64(&mut image, RELA_OFFSET, 0x40_0100),
        4 => write_u64(&mut image, DYNAMIC_OFFSET + 2 * DYNAMIC_ENTRY_SIZE, DT_RELA),
        5 => write_u64(&mut image, DYNAMIC_OFFSET + 8, 16),
        6 => write_u64(
            &mut image,
            RELA_OFFSET + RELA_ENTRY_SIZE + 8,
            (1_u64 << 32) | 7,
        ),
        7 => write_u64(
            &mut image,
            RELA_OFFSET + RELA_ENTRY_SIZE + 8,
            (2_u64 << 32) | 6,
        ),
        8 => write_u64(&mut image, RELA_OFFSET + RELA_ENTRY_SIZE, 0x40_6100),
        9 => image[SYMTAB_OFFSET] = 1,
        10 => write_u32(&mut image, SYMTAB_OFFSET + SYMBOL_ENTRY_SIZE, 25),
        11 => image[SYMTAB_OFFSET + SYMBOL_ENTRY_SIZE + 4] = 0x11,
        12 => write_u16(&mut image, SYMTAB_OFFSET + SYMBOL_ENTRY_SIZE + 6, 1),
        13 => write_u64(&mut image, SYMTAB_OFFSET + SYMBOL_ENTRY_SIZE + 8, 0x40_5000),
        14 => image[STRTAB_OFFSET] = b'x',
        15 => write_u64(&mut image, RELA_OFFSET + RELA_ENTRY_SIZE + 16, 1),
        16 => write_u64(
            &mut image,
            RELA_OFFSET + 2 * RELA_ENTRY_SIZE + 8,
            (1_u64 << 32) | 6,
        ),
        17 => write_u64(
            &mut image,
            RELA_OFFSET + 2 * RELA_ENTRY_SIZE + 8,
            (2_u64 << 32) | 7,
        ),
        18 => write_u64(&mut image, RELA_OFFSET + 2 * RELA_ENTRY_SIZE, 0x40_6108),
        19 => write_u64(&mut image, RELA_OFFSET + 2 * RELA_ENTRY_SIZE + 16, 1),
        20 => write_u64(&mut image, DYNAMIC_OFFSET + 7 * DYNAMIC_ENTRY_SIZE, DT_RELA),
        21 => write_u32(&mut image, HASH_OFFSET, 0),
        22 => write_u32(&mut image, HASH_OFFSET + 4, 3),
        23 => write_u32(&mut image, HASH_OFFSET + 8, 0),
        24 => write_u32(&mut image, HASH_OFFSET + 16, 1),
        25 => write_u64(&mut image, DYNAMIC_OFFSET + 8 * DYNAMIC_ENTRY_SIZE, DT_RELA),
        26 => write_u64(&mut image, DYNAMIC_OFFSET + 8 * DYNAMIC_ENTRY_SIZE + 8, 0),
        27 => write_u64(&mut image, DYNAMIC_OFFSET + 8 * DYNAMIC_ENTRY_SIZE + 8, 25),
        28 => write_u64(&mut image, DYNAMIC_OFFSET + 9 * DYNAMIC_ENTRY_SIZE, DT_RELA),
        29 => write_u64(&mut image, DYNAMIC_OFFSET + 10 * DYNAMIC_ENTRY_SIZE + 8, 0),
        30 => write_u64(&mut image, DYNAMIC_OFFSET + 11 * DYNAMIC_ENTRY_SIZE + 8, 0),
        31 => write_u64(
            &mut image,
            DYNAMIC_OFFSET + 9 * DYNAMIC_ENTRY_SIZE + 8,
            0x3f_f000 + RELA_OFFSET as u64,
        ),
        _ => return false,
    }
    parse(&image).is_none()
}

#[inline(never)]
fn verify_oversized_writable_plan_rejected() -> bool {
    let mut oversized = fixture();
    write_u64(
        &mut oversized,
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE + 40,
        (MAX_OTHER_SEGMENT_PAGES * PAGE_SIZE + 1) as u64,
    );
    write_u16(&mut oversized, 56, MIN_LOAD_SEGMENT_COUNT as u16);
    parse(&oversized).is_some_and(|image| plan(&image).is_none())
}

#[inline(never)]
fn verify_page_aliasing_plan_rejected() -> bool {
    let static_bytes = fixture();
    let Some(mut static_image) = parse(&static_bytes) else {
        crate::serial_write_bytes(b"\r\nginux: page alias static fixture parse failed");
        return false;
    };
    static_image.read_only = Some(Segment {
        bytes: &static_bytes[..16],
        file_offset: 0,
        virtual_address: 0x40_6200,
        memory_size: 16,
        flags: PF_READ,
    });
    let static_rejected = plan(&static_image).is_none();

    let dynamic_bytes = compact_position_independent_fixture();
    let Some(mut dynamic_image) = parse(&dynamic_bytes) else {
        crate::serial_write_bytes(b"\r\nginux: page alias dynamic fixture parse failed");
        return false;
    };
    dynamic_image.read_only = Some(Segment {
        bytes: &dynamic_bytes[..16],
        file_offset: 0,
        virtual_address: 0x3100,
        memory_size: 16,
        flags: PF_READ,
    });
    let dynamic_rejected = plan_position_independent(&dynamic_image, 0x60_0000).is_none();
    if !static_rejected {
        crate::serial_write_bytes(b"\r\nginux: static page alias accepted");
    }
    if !dynamic_rejected {
        crate::serial_write_bytes(b"\r\nginux: dynamic page alias accepted");
    }
    static_rejected && dynamic_rejected
}

pub(crate) fn verify_linked_probe() -> bool {
    let Some(image) = parse(LINKED_PROBE) else {
        return false;
    };
    let Some(plan) = plan(&image) else {
        return false;
    };
    let Some(read_only) = plan.read_only else {
        return false;
    };
    let mut page = [0xa5_u8; PAGE_SIZE];
    let mut executable_tail_page = [0xa5_u8; PAGE_SIZE];
    let mut executable_third_page = [0xa5_u8; PAGE_SIZE];
    let mut executable_fourth_page = [0xa5_u8; PAGE_SIZE];
    let first_page_code = PAGE_SIZE - 0x100;
    let second_page_code = (PROBE_CODE.len() - first_page_code).min(PAGE_SIZE);
    let third_page_offset = first_page_code + second_page_code;
    let third_page_code = (PROBE_CODE.len() - third_page_offset).min(PAGE_SIZE);
    let fourth_page_offset = third_page_offset + third_page_code;
    let fourth_page_code = (PROBE_CODE.len() - fourth_page_offset).min(PAGE_SIZE);
    let fifth_page_offset = fourth_page_offset + fourth_page_code;
    let fifth_page_code = PROBE_CODE.len() - fifth_page_offset;
    let read_only_valid = materialize(read_only, &mut page)
        && page[..4] == *b"\x7fELF"
        && materialized_page_matches(read_only, 0, &page);
    let executable_valid = materialize(plan.executable, &mut page)
        && page[..0x100].iter().all(|byte| *byte == 0)
        && page[0x100..] == PROBE_CODE[..first_page_code]
        && materialized_page_matches(plan.executable, 0, &page)
        && materialize_page(plan.executable, 1, &mut executable_tail_page)
        && executable_tail_page[..second_page_code]
            == PROBE_CODE[first_page_code..first_page_code + second_page_code]
        && executable_tail_page[second_page_code..]
            .iter()
            .all(|byte| *byte == 0)
        && materialized_page_matches(plan.executable, 1, &executable_tail_page);
    let executable_valid = executable_valid
        && materialize_page(plan.executable, 2, &mut executable_third_page)
        && executable_third_page[..third_page_code]
            == PROBE_CODE[third_page_offset..third_page_offset + third_page_code]
        && executable_third_page[third_page_code..]
            .iter()
            .all(|byte| *byte == 0)
        && materialized_page_matches(plan.executable, 2, &executable_third_page);
    let executable_valid = executable_valid
        && materialize_page(plan.executable, 3, &mut executable_fourth_page)
        && executable_fourth_page[..fourth_page_code]
            == PROBE_CODE[fourth_page_offset..fourth_page_offset + fourth_page_code]
        && executable_fourth_page[fourth_page_code..]
            .iter()
            .all(|byte| *byte == 0)
        && materialized_page_matches(plan.executable, 3, &executable_fourth_page);
    let executable_valid = executable_valid
        && materialize_page(plan.executable, 4, &mut page)
        && page[..fifth_page_code] == PROBE_CODE[fifth_page_offset..]
        && page[fifth_page_code..].iter().all(|byte| *byte == 0)
        && materialized_page_matches(plan.executable, 4, &page);
    let writable_valid = materialize(plan.writable, &mut page)
        && page[..8] == *b"PROBE!!\n"
        && page[8..].iter().all(|byte| *byte == 0)
        && materialize_page(plan.writable, 1, &mut page)
        && page.iter().all(|byte| *byte == 0);
    image.entry == 0x40_0100
        && image.program_headers == 0x3f_f040
        && image.program_header_count == 6
        && image.executable.virtual_address == 0x40_0100
        && image.executable.bytes.starts_with(PROBE_CODE)
        && image.executable.bytes[PROBE_CODE.len()..]
            .iter()
            .all(|byte| *byte == 0)
        && image
            .read_only
            .is_some_and(|segment| segment.virtual_address == 0x3f_f000)
        && image.writable.virtual_address == 0x40_5000
        && image.writable.bytes.len() == 4608
        && image.writable.bytes[..8] == *b"PROBE!!\n"
        && image.writable.bytes[8..].iter().all(|byte| *byte == 0)
        && image.writable.memory_size == 4608
        && image
            .relro
            .is_some_and(|range| range.virtual_address == 0x40_6000 && range.memory_size == 512)
        && plan
            .relro
            .is_some_and(|range| range.virtual_address == 0x40_6000 && range.memory_size == 512)
        && plan.executable.virtual_page == 0x40_0000
        && plan.executable.virtual_address == 0x40_0100
        && plan.executable.page_count == 5
        && read_only.virtual_page == 0x3f_f000
        && plan.writable.page_count == 2
        && read_only_valid
        && executable_valid
        && writable_valid
        && crate::address_space::verify_linux_elf_mapping(&plan, 1, 1)
}

pub(crate) fn verify_mapped_load_plan(protected_start: u64, protected_size: usize) -> bool {
    let image = fixture();
    let Some(parsed) = parse(&image) else {
        return false;
    };
    let Some(plan) = plan(&parsed) else {
        return false;
    };
    crate::address_space::verify_linux_elf_mapping(&plan, protected_start, protected_size)
}

pub(crate) fn verify_mapped_interpreted_plan(protected_start: u64, protected_size: usize) -> bool {
    const PATH: &[u8] = b"/lib64/ld-ginux.so";
    const DEPENDENCY: &[u8] = b"libginux.so";
    const AUXILIARY: &[u8] = b"libginux-aux.so";
    let interpreter_bytes = LINKED_DYNAMIC_INTERPRETER;
    let Some(main) = parse(LINKED_DYNAMIC_GRAPH_MAIN) else {
        crate::serial_write_bytes(b"\r\nginux: linked dynamic main parse failed");
        return false;
    };
    let Some(dependency) = parse(LINKED_DYNAMIC_DEPENDENCY) else {
        crate::serial_write_bytes(b"\r\nginux: linked dynamic dependency parse failed");
        return false;
    };
    let Some(interpreter) = parse(interpreter_bytes) else {
        crate::serial_write_bytes(b"\r\nginux: linked dynamic interpreter parse failed");
        return false;
    };
    let authority = InterpreterAuthority {
        path: PATH,
        sha256: crate::crypto::sha256(interpreter_bytes),
    };
    let dependency_authorities = [
        DependencyAuthority {
            name: DEPENDENCY,
            sha256: crate::crypto::sha256(LINKED_DYNAMIC_DEPENDENCY),
            load_bias: 0x61_0000,
        },
        DependencyAuthority {
            name: AUXILIARY,
            sha256: crate::crypto::sha256(LINKED_DYNAMIC_AUXILIARY),
            load_bias: 0x63_0000,
        },
    ];
    if main.dynamic_symbol != dependency.dynamic_symbol {
        crate::serial_write_bytes(b"\r\nginux: linked dynamic symbol mismatch");
        return false;
    }
    if main.interpreter != Some(PATH)
        || main.needed_libraries != [Some(DEPENDENCY), Some(AUXILIARY)]
        || dependency.soname != Some(DEPENDENCY)
    {
        crate::serial_write_bytes(b"\r\nginux: linked dynamic identity mismatch");
        return false;
    }
    if interpreter.entry != 0x200
        || interpreter.program_header_count != 5
        || interpreter.interpreter.is_some()
        || interpreter.needed_libraries != [None; 2]
        || interpreter.soname.is_some()
        || !interpreter.dynamic_relocations.is_some_and(|dynamic| {
            dynamic.relocation_count == 1
                && dynamic.symbol_table_address.is_some()
                && dynamic.string_table_address.is_some()
                && dynamic.string_table_size == 1
                && dynamic.hash_table_address.is_some()
                && dynamic.hash_is_gnu
                && dynamic.symbol_relocation.is_none()
                && dynamic.jump_slot_relocation.is_none()
                && !dynamic.symbol_defined
        })
    {
        crate::serial_write_bytes(b"\r\nginux: linked dynamic interpreter metadata mismatch");
        return false;
    }
    if plan_position_independent_at(&main, 0x3f_f000, true).is_none() {
        crate::serial_write_bytes(b"\r\nginux: linked PIE main planning failed");
        return false;
    }
    if plan_position_independent(&dependency, 0x61_0000).is_none() {
        crate::serial_write_bytes(b"\r\nginux: linked DSO planning failed");
        return false;
    }
    let Some(plan) = plan_authenticated_pie_dependency_graph(
        &main,
        0x3f_f000,
        &authority,
        interpreter_bytes,
        0x60_0000,
        &dependency_authorities,
        [LINKED_DYNAMIC_DEPENDENCY, LINKED_DYNAMIC_AUXILIARY],
    ) else {
        crate::serial_write_bytes(b"\r\nginux: linked dynamic dependency plan failed");
        return false;
    };
    if plan.process.interpreter.executable.page_count != 2
        || plan.dependencies[0].executable.page_count != 2
        || plan.dependencies[0].writable.page_count != 1
        || !plan.dependencies[0]
            .read_only
            .is_some_and(|segment| segment.page_count == 1)
        || plan.dependencies[1].executable.page_count != 1
        || plan.dependencies[1].writable.page_count != 1
        || plan.dependencies.iter().any(|dependency| {
            dependency
                .dynamic_relocations
                .is_none_or(|dynamic| dynamic.init_array != Some(dynamic.relocation_target))
        })
    {
        crate::serial_write_bytes(b"\r\nginux: linked DSO page-contract fixture mismatch");
        return false;
    }
    let main_symbols_unresolved = plan
        .process
        .main
        .dynamic_relocations
        .is_some_and(|dynamic| {
            !dynamic.symbol_defined
                && dynamic
                    .symbol_relocation
                    .is_some_and(|relocation| relocation.symbol_value == 0)
                && dynamic
                    .jump_slot_relocation
                    .is_some_and(|relocation| relocation.symbol_value == 0)
                && !dynamic.secondary_symbol_defined
                && dynamic
                    .secondary_symbol_relocation
                    .is_some_and(|relocation| relocation.symbol_value == 0)
                && dynamic
                    .secondary_jump_slot_relocation
                    .is_some_and(|relocation| relocation.symbol_value == 0)
        });
    if !main_symbols_unresolved {
        crate::serial_write_bytes(
            b"\r\nginux: linked PIE symbols resolved before dependency binding",
        );
        return false;
    }
    crate::address_space::verify_interpreted_elf_mapping(&plan, protected_start, protected_size)
}

fn interpreted_main_fixture() -> [u8; INTERPRETED_FIXTURE_SIZE] {
    const PATH: &[u8] = b"/lib64/ld-ginux.so\0";
    const EXECUTABLE_OFFSET: usize = 0x200;
    const RELA_OFFSET: usize = ELF_HEADER_SIZE + 6 * PROGRAM_HEADER_SIZE;
    const STRING_TABLE: &[u8] = b"\0ginux_probe\0libginux.so\0";
    const PATH_OFFSET: usize = 0x1e0;
    const DYNAMIC_OFFSET: usize = INTERPRETED_DATA_OFFSET + 8;
    const SYMTAB_OFFSET: usize = DYNAMIC_OFFSET + 13 * DYNAMIC_ENTRY_SIZE;
    const STRTAB_OFFSET: usize = SYMTAB_OFFSET + 2 * SYMBOL_ENTRY_SIZE;
    const HASH_OFFSET: usize = (STRTAB_OFFSET + STRING_TABLE.len() + 3) & !3;
    let mut bytes = [0_u8; INTERPRETED_FIXTURE_SIZE];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, ET_EXEC);
    write_u16(&mut bytes, 18, EM_X86_64);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, 0x40_0100);
    write_u64(&mut bytes, 32, ELF_HEADER_SIZE as u64);
    write_u16(&mut bytes, 52, ELF_HEADER_SIZE as u16);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_SIZE as u16);
    write_u16(&mut bytes, 56, 6);
    let header_load_size = (RELA_OFFSET + 3 * RELA_ENTRY_SIZE) as u64;
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE,
        0,
        0x3f_f000,
        header_load_size,
        header_load_size,
        PF_READ,
    );
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE,
        EXECUTABLE_OFFSET as u64,
        0x40_0100,
        PROBE_CODE.len() as u64,
        (5 * PAGE_SIZE - 0x100) as u64,
        PF_READ | PF_EXECUTE,
    );
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE + 2 * PROGRAM_HEADER_SIZE,
        INTERPRETED_DATA_OFFSET as u64,
        0x40_5000,
        (HASH_OFFSET + 20 - INTERPRETED_DATA_OFFSET) as u64,
        (PAGE_SIZE + 512) as u64,
        PF_READ | PF_WRITE,
    );
    write_relro(
        &mut bytes,
        ELF_HEADER_SIZE + 3 * PROGRAM_HEADER_SIZE,
        0x40_6000,
        512,
    );
    write_interpreter(
        &mut bytes,
        ELF_HEADER_SIZE + 4 * PROGRAM_HEADER_SIZE,
        PATH_OFFSET,
        PATH,
    );
    write_dynamic(
        &mut bytes,
        ELF_HEADER_SIZE + 5 * PROGRAM_HEADER_SIZE,
        DYNAMIC_OFFSET,
        0x40_5008,
        13 * DYNAMIC_ENTRY_SIZE,
    );
    write_u64(&mut bytes, RELA_OFFSET, 0x40_6100);
    write_u64(&mut bytes, RELA_OFFSET + 8, R_X86_64_RELATIVE);
    write_u64(&mut bytes, RELA_OFFSET + 16, 0x40_0100);
    write_u64(&mut bytes, RELA_OFFSET + RELA_ENTRY_SIZE, 0x40_6108);
    write_u64(
        &mut bytes,
        RELA_OFFSET + RELA_ENTRY_SIZE + 8,
        (1_u64 << 32) | u64::from(R_X86_64_GLOB_DAT),
    );
    write_u64(&mut bytes, RELA_OFFSET + RELA_ENTRY_SIZE + 16, 0);
    write_u64(&mut bytes, RELA_OFFSET + 2 * RELA_ENTRY_SIZE, 0x40_6110);
    write_u64(
        &mut bytes,
        RELA_OFFSET + 2 * RELA_ENTRY_SIZE + 8,
        (1_u64 << 32) | u64::from(R_X86_64_JUMP_SLOT),
    );
    write_u64(&mut bytes, RELA_OFFSET + 2 * RELA_ENTRY_SIZE + 16, 0);
    write_u32(&mut bytes, SYMTAB_OFFSET + SYMBOL_ENTRY_SIZE, 1);
    bytes[SYMTAB_OFFSET + SYMBOL_ENTRY_SIZE + 4] = 0x12;
    bytes[STRTAB_OFFSET..STRTAB_OFFSET + STRING_TABLE.len()].copy_from_slice(STRING_TABLE);
    write_u32(&mut bytes, HASH_OFFSET, 1);
    write_u32(&mut bytes, HASH_OFFSET + 4, 2);
    write_u32(&mut bytes, HASH_OFFSET + 8, 1);
    write_u32(&mut bytes, HASH_OFFSET + 12, 0);
    write_u32(&mut bytes, HASH_OFFSET + 16, 0);
    for (index, (tag, value)) in [
        (DT_SYMENT, SYMBOL_ENTRY_SIZE as u64),
        (DT_RELA, 0x3f_f000 + RELA_OFFSET as u64),
        (
            DT_STRTAB,
            0x40_5000 + (STRTAB_OFFSET - INTERPRETED_DATA_OFFSET) as u64,
        ),
        (DT_RELAENT, RELA_ENTRY_SIZE as u64),
        (
            DT_SYMTAB,
            0x40_5000 + (SYMTAB_OFFSET - INTERPRETED_DATA_OFFSET) as u64,
        ),
        (DT_RELASZ, (2 * RELA_ENTRY_SIZE) as u64),
        (DT_STRSZ, STRING_TABLE.len() as u64),
        (
            DT_HASH,
            0x40_5000 + (HASH_OFFSET - INTERPRETED_DATA_OFFSET) as u64,
        ),
        (DT_NEEDED, 13),
        (
            DT_JMPREL,
            0x3f_f000 + (RELA_OFFSET + 2 * RELA_ENTRY_SIZE) as u64,
        ),
        (DT_PLTRELSZ, RELA_ENTRY_SIZE as u64),
        (DT_PLTREL, DT_RELA),
        (DT_NULL, 0),
    ]
    .into_iter()
    .enumerate()
    {
        write_u64(&mut bytes, DYNAMIC_OFFSET + index * DYNAMIC_ENTRY_SIZE, tag);
        write_u64(
            &mut bytes,
            DYNAMIC_OFFSET + index * DYNAMIC_ENTRY_SIZE + 8,
            value,
        );
    }
    bytes[EXECUTABLE_OFFSET..EXECUTABLE_OFFSET + PROBE_CODE.len()].copy_from_slice(PROBE_CODE);
    bytes[INTERPRETED_DATA_OFFSET..INTERPRETED_DATA_OFFSET + 8].copy_from_slice(b"PROBE!!\n");
    bytes
}

fn fixture() -> [u8; FIXTURE_SIZE] {
    let mut bytes = [0_u8; FIXTURE_SIZE];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, ET_EXEC);
    write_u16(&mut bytes, 18, EM_X86_64);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, 0x40_0100);
    write_u64(&mut bytes, 32, ELF_HEADER_SIZE as u64);
    write_u16(&mut bytes, 52, ELF_HEADER_SIZE as u16);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_SIZE as u16);
    write_u16(&mut bytes, 56, (MIN_LOAD_SEGMENT_COUNT + 1) as u16);
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE,
        0,
        0x40_0000,
        (0x100 + PROBE_CODE.len()) as u64,
        FIXTURE_EXECUTABLE_MEMORY_SIZE,
        PF_READ | PF_EXECUTE,
    );
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE,
        FIXTURE_DATA_OFFSET as u64,
        0x40_5000,
        8,
        (PAGE_SIZE + 512) as u64,
        PF_READ | PF_WRITE,
    );
    let stack_header = ELF_HEADER_SIZE + 2 * PROGRAM_HEADER_SIZE;
    write_relro(&mut bytes, stack_header, 0x40_6000, 512);
    bytes[0x100..0x100 + PROBE_CODE.len()].copy_from_slice(PROBE_CODE);
    bytes[FIXTURE_DATA_OFFSET..FIXTURE_SIZE].copy_from_slice(b"PROBE!!\n");
    bytes
}

fn compact_position_independent_fixture() -> [u8; 2048] {
    const DATA_OFFSET: usize = 0x400;
    const DYNAMIC_OFFSET: usize = DATA_OFFSET + 8;
    const RELA_OFFSET: usize = DYNAMIC_OFFSET + 4 * DYNAMIC_ENTRY_SIZE;
    let mut bytes = [0_u8; 2048];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, ET_DYN);
    write_u16(&mut bytes, 18, EM_X86_64);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, 0x100);
    write_u64(&mut bytes, 32, ELF_HEADER_SIZE as u64);
    write_u16(&mut bytes, 52, ELF_HEADER_SIZE as u16);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_SIZE as u16);
    write_u16(&mut bytes, 56, (MIN_LOAD_SEGMENT_COUNT + 1) as u16);
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE,
        0,
        0,
        (0x100 + INTERPRETER_CODE.len()) as u64,
        (0x100 + INTERPRETER_CODE.len()) as u64,
        PF_READ | PF_EXECUTE,
    );
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE,
        DATA_OFFSET as u64,
        0x3000,
        (8 + 4 * DYNAMIC_ENTRY_SIZE + RELA_ENTRY_SIZE) as u64,
        0x100,
        PF_READ | PF_WRITE,
    );
    write_dynamic(
        &mut bytes,
        ELF_HEADER_SIZE + 2 * PROGRAM_HEADER_SIZE,
        DYNAMIC_OFFSET,
        0x3008,
        4 * DYNAMIC_ENTRY_SIZE,
    );
    for (index, (tag, value)) in [
        (DT_RELA, 0x3048),
        (DT_RELASZ, RELA_ENTRY_SIZE as u64),
        (DT_RELAENT, RELA_ENTRY_SIZE as u64),
        (DT_NULL, 0),
    ]
    .into_iter()
    .enumerate()
    {
        write_u64(&mut bytes, DYNAMIC_OFFSET + index * DYNAMIC_ENTRY_SIZE, tag);
        write_u64(
            &mut bytes,
            DYNAMIC_OFFSET + index * DYNAMIC_ENTRY_SIZE + 8,
            value,
        );
    }
    write_u64(&mut bytes, RELA_OFFSET, 0x3080);
    write_u64(&mut bytes, RELA_OFFSET + 8, R_X86_64_RELATIVE);
    write_u64(&mut bytes, RELA_OFFSET + 16, 0x1234);
    bytes[0x100..0x100 + INTERPRETER_CODE.len()].copy_from_slice(INTERPRETER_CODE);
    bytes[DATA_OFFSET..DATA_OFFSET + 8].copy_from_slice(b"DYNAMIC!");
    bytes
}

fn compact_dependency_fixture() -> [u8; 1024] {
    const DATA_OFFSET: usize = 0x200;
    const DYNAMIC_OFFSET: usize = DATA_OFFSET + 8;
    const SYMTAB_OFFSET: usize = DYNAMIC_OFFSET + 13 * DYNAMIC_ENTRY_SIZE;
    const STRTAB_OFFSET: usize = SYMTAB_OFFSET + 2 * SYMBOL_ENTRY_SIZE;
    const STRING_TABLE: &[u8] = b"\0ginux_probe\0libginux.so\0";
    const HASH_OFFSET: usize = (STRTAB_OFFSET + STRING_TABLE.len() + 3) & !3;
    const RELA_OFFSET: usize = (HASH_OFFSET + 32 + 7) & !7;
    let mut bytes = [0_u8; 1024];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, ET_DYN);
    write_u16(&mut bytes, 18, EM_X86_64);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, 0x100);
    write_u64(&mut bytes, 32, ELF_HEADER_SIZE as u64);
    write_u16(&mut bytes, 52, ELF_HEADER_SIZE as u16);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_SIZE as u16);
    write_u16(&mut bytes, 56, 3);
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE,
        0,
        0,
        (0x100 + DEPENDENCY_CODE.len()) as u64,
        0x200,
        PF_READ | PF_EXECUTE,
    );
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE,
        DATA_OFFSET as u64,
        0x3000,
        (RELA_OFFSET + 3 * RELA_ENTRY_SIZE - DATA_OFFSET) as u64,
        0x200,
        PF_READ | PF_WRITE,
    );
    write_dynamic(
        &mut bytes,
        ELF_HEADER_SIZE + 2 * PROGRAM_HEADER_SIZE,
        DYNAMIC_OFFSET,
        0x3008,
        13 * DYNAMIC_ENTRY_SIZE,
    );
    for (index, (target, kind, symbol, addend)) in [
        (0x31c0, R_X86_64_RELATIVE as u32, 0_u32, 0x100),
        (0x31c8, R_X86_64_GLOB_DAT, 1, 0),
        (0x31d0, R_X86_64_JUMP_SLOT, 1, 0),
    ]
    .into_iter()
    .enumerate()
    {
        let offset = RELA_OFFSET + index * RELA_ENTRY_SIZE;
        write_u64(&mut bytes, offset, target);
        write_u64(
            &mut bytes,
            offset + 8,
            (u64::from(symbol) << 32) | u64::from(kind),
        );
        write_u64(&mut bytes, offset + 16, addend);
    }
    write_u32(&mut bytes, SYMTAB_OFFSET + SYMBOL_ENTRY_SIZE, 1);
    bytes[SYMTAB_OFFSET + SYMBOL_ENTRY_SIZE + 4] = 0x12;
    write_u16(&mut bytes, SYMTAB_OFFSET + SYMBOL_ENTRY_SIZE + 6, 1);
    write_u64(&mut bytes, SYMTAB_OFFSET + SYMBOL_ENTRY_SIZE + 8, 0x100);
    write_u64(
        &mut bytes,
        SYMTAB_OFFSET + SYMBOL_ENTRY_SIZE + 16,
        DEPENDENCY_CODE.len() as u64,
    );
    bytes[STRTAB_OFFSET..STRTAB_OFFSET + STRING_TABLE.len()].copy_from_slice(STRING_TABLE);
    let symbol_hash = gnu_hash(b"ginux_probe");
    let bloom = (1_u64 << (symbol_hash % 64)) | (1_u64 << ((symbol_hash >> 5) % 64));
    write_u32(&mut bytes, HASH_OFFSET, 1);
    write_u32(&mut bytes, HASH_OFFSET + 4, 1);
    write_u32(&mut bytes, HASH_OFFSET + 8, 1);
    write_u32(&mut bytes, HASH_OFFSET + 12, 5);
    write_u64(&mut bytes, HASH_OFFSET + 16, bloom);
    write_u32(&mut bytes, HASH_OFFSET + 24, 1);
    write_u32(&mut bytes, HASH_OFFSET + 28, symbol_hash | 1);
    for (index, (tag, value)) in [
        (DT_SYMENT, SYMBOL_ENTRY_SIZE as u64),
        (DT_RELA, 0x3000 + (RELA_OFFSET - DATA_OFFSET) as u64),
        (DT_STRTAB, 0x3000 + (STRTAB_OFFSET - DATA_OFFSET) as u64),
        (DT_RELAENT, RELA_ENTRY_SIZE as u64),
        (DT_SYMTAB, 0x3000 + (SYMTAB_OFFSET - DATA_OFFSET) as u64),
        (DT_RELASZ, (2 * RELA_ENTRY_SIZE) as u64),
        (DT_STRSZ, STRING_TABLE.len() as u64),
        (DT_GNU_HASH, 0x3000 + (HASH_OFFSET - DATA_OFFSET) as u64),
        (DT_SONAME, 13),
        (
            DT_JMPREL,
            0x3000 + (RELA_OFFSET + 2 * RELA_ENTRY_SIZE - DATA_OFFSET) as u64,
        ),
        (DT_PLTRELSZ, RELA_ENTRY_SIZE as u64),
        (DT_PLTREL, DT_RELA),
        (DT_NULL, 0),
    ]
    .into_iter()
    .enumerate()
    {
        write_u64(&mut bytes, DYNAMIC_OFFSET + index * DYNAMIC_ENTRY_SIZE, tag);
        write_u64(
            &mut bytes,
            DYNAMIC_OFFSET + index * DYNAMIC_ENTRY_SIZE + 8,
            value,
        );
    }
    bytes[0x100..0x100 + DEPENDENCY_CODE.len()].copy_from_slice(DEPENDENCY_CODE);
    bytes[DATA_OFFSET..DATA_OFFSET + 8].copy_from_slice(b"LIBRARY!");
    bytes
}

fn write_gnu_stack(bytes: &mut [u8], header: usize) {
    bytes[header..header + PROGRAM_HEADER_SIZE].fill(0);
    write_u32(bytes, header, PT_GNU_STACK);
    write_u32(bytes, header + 4, PF_READ | PF_WRITE);
    write_u64(bytes, header + 48, 16);
}

fn write_phdr(bytes: &mut [u8], header: usize, program_header_count: usize) {
    bytes[header..header + PROGRAM_HEADER_SIZE].fill(0);
    write_u32(bytes, header, PT_PHDR);
    write_u32(bytes, header + 4, PF_READ);
    write_u64(bytes, header + 8, ELF_HEADER_SIZE as u64);
    write_u64(bytes, header + 16, 0x40_0040);
    let table_size = (program_header_count * PROGRAM_HEADER_SIZE) as u64;
    write_u64(bytes, header + 32, table_size);
    write_u64(bytes, header + 40, table_size);
    write_u64(bytes, header + 48, 8);
}

fn write_interpreter(bytes: &mut [u8], header: usize, offset: usize, path: &[u8]) {
    bytes[header..header + PROGRAM_HEADER_SIZE].fill(0);
    write_u32(bytes, header, PT_INTERP);
    write_u32(bytes, header + 4, PF_READ);
    write_u64(bytes, header + 8, offset as u64);
    write_u64(bytes, header + 32, path.len() as u64);
    write_u64(bytes, header + 40, path.len() as u64);
    write_u64(bytes, header + 48, 1);
    bytes[offset..offset + path.len()].copy_from_slice(path);
}

fn write_dynamic(
    bytes: &mut [u8],
    header: usize,
    offset: usize,
    virtual_address: u64,
    size: usize,
) {
    bytes[header..header + PROGRAM_HEADER_SIZE].fill(0);
    write_u32(bytes, header, PT_DYNAMIC);
    write_u32(bytes, header + 4, PF_READ | PF_WRITE);
    write_u64(bytes, header + 8, offset as u64);
    write_u64(bytes, header + 16, virtual_address);
    write_u64(bytes, header + 24, virtual_address);
    write_u64(bytes, header + 32, size as u64);
    write_u64(bytes, header + 40, size as u64);
    write_u64(bytes, header + 48, 8);
}

fn write_relro(bytes: &mut [u8], header: usize, virtual_address: u64, memory_size: u64) {
    bytes[header..header + PROGRAM_HEADER_SIZE].fill(0);
    write_u32(bytes, header, PT_GNU_RELRO);
    write_u32(bytes, header + 4, PF_READ);
    write_u64(bytes, header + 16, virtual_address);
    write_u64(bytes, header + 24, virtual_address);
    write_u64(bytes, header + 40, memory_size);
    write_u64(bytes, header + 48, 8);
}

fn swap_program_headers(bytes: &mut [u8], first: usize, second: usize) {
    let mut saved = [0_u8; PROGRAM_HEADER_SIZE];
    saved.copy_from_slice(&bytes[first..first + PROGRAM_HEADER_SIZE]);
    bytes.copy_within(second..second + PROGRAM_HEADER_SIZE, first);
    bytes[second..second + PROGRAM_HEADER_SIZE].copy_from_slice(&saved);
}

fn write_segment(
    bytes: &mut [u8],
    header: usize,
    offset: u64,
    virtual_address: u64,
    file_size: u64,
    memory_size: u64,
    flags: u32,
) {
    write_u32(bytes, header, PT_LOAD);
    write_u32(bytes, header + 4, flags);
    write_u64(bytes, header + 8, offset);
    write_u64(bytes, header + 16, virtual_address);
    write_u64(bytes, header + 32, file_size);
    write_u64(bytes, header + 40, memory_size);
    write_u64(bytes, header + 48, 8);
}

fn rejects(bytes: &[u8], offset: usize, value: u8) -> bool {
    let mut candidate = fixture();
    candidate[..bytes.len()].copy_from_slice(bytes);
    candidate[offset] = value;
    parse(&candidate).is_none()
}

fn rejects_u16(bytes: &[u8], offset: usize, value: u16) -> bool {
    let mut candidate = fixture();
    candidate[..bytes.len()].copy_from_slice(bytes);
    write_u16(&mut candidate, offset, value);
    parse(&candidate).is_none()
}

fn rejects_u32(bytes: &[u8], offset: usize, value: u32) -> bool {
    let mut candidate = fixture();
    candidate[..bytes.len()].copy_from_slice(bytes);
    write_u32(&mut candidate, offset, value);
    parse(&candidate).is_none()
}

fn rejects_u64(bytes: &[u8], offset: usize, value: u64) -> bool {
    let mut candidate = fixture();
    candidate[..bytes.len()].copy_from_slice(bytes);
    write_u64(&mut candidate, offset, value);
    parse(&candidate).is_none()
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        bytes.get(offset..offset + 8)?.try_into().ok()?,
    ))
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
