ENTRY(ginux_linux_interpreter)

PHDRS
{
    phdr PT_PHDR PHDRS FLAGS(4);
    text PT_LOAD FILEHDR PHDRS FLAGS(5);
    data PT_LOAD FLAGS(6);
    dynamic PT_DYNAMIC FLAGS(6);
    stack PT_GNU_STACK FLAGS(6);
}

SECTIONS
{
    . = SIZEOF_HEADERS;
    .dynsym : { *(.dynsym) } :text
    .gnu.hash : { *(.gnu.hash) } :text
    .hash : { *(.hash) } :text
    .dynstr : { *(.dynstr) } :text
    .rela.dyn : { *(.rela.dyn) } :text

    . = 0x200;
    .text : { *(.text) } :text
    .text.discovery : { *(.text.discovery) } :text
    .rodata.loader : { *(.rodata.loader) } :text
    ASSERT(. <= 0x1000, "Linux interpreter discovery exceeds its RX page")

    . = 0x3000;
    .data : { *(.data) } :data
    .dynamic : { *(.dynamic) } :data :dynamic
    .data.rel.ro : { *(.data.rel.ro) } :data

    /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame*) }
}
