ENTRY(ginux_linux_probe)

PHDRS
{
    phdr PT_PHDR PHDRS FLAGS(4);
    headers PT_LOAD FILEHDR PHDRS FLAGS(4);
    text PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
    dynamic PT_DYNAMIC FLAGS(6);
    interp PT_INTERP FLAGS(4);
    relro PT_GNU_RELRO FLAGS(4);
    stack PT_GNU_STACK FLAGS(6);
}

SECTIONS
{
    . = SIZEOF_HEADERS;
    .interp : { *(.interp) } :headers :interp
    .dynsym : { *(.dynsym) } :headers
    .gnu.hash : { *(.gnu.hash) } :headers
    .dynstr : { *(.dynstr) } :headers
    .rela.dyn : { *(.rela.dyn) } :headers
    .rela.plt : { *(.rela.plt) } :headers

    . = 0x1000;
    .text.file_mmap : { *(.text.file_mmap) } :text
    ASSERT(SIZEOF(.text.file_mmap) <= 0x100, "Linux file mmap proof exceeds reserved prefix")

    . = 0x1100;
    .text : { *(.text) *(.text.*) } :text

    . = 0x5000;
    .data : { *(.data) } :data
    .bss : { *(.bss) } :data

    . = 0x6000;
    .data.rel.ro.head : { *(.data.rel.ro.head) } :data :relro
    .preinit_array : { *(.preinit_array) } :data :relro
    .init_array : { *(.init_array) } :data :relro
    .data.rel.ro.tail : { *(.data.rel.ro.tail) } :data :relro
    .data.rel.ro : { *(.data.rel.ro) } :data :relro
    .dynamic : { *(.dynamic) } :data :dynamic :relro
    .got : { *(.got) } :data :relro
    .got.plt : { *(.got.plt) } :data :relro

    /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame*) }
}
