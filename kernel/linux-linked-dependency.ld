ENTRY(ginux_dependency_symbol)

PHDRS
{
    text PT_LOAD FLAGS(5);
    rodata PT_LOAD FLAGS(4);
    data PT_LOAD FLAGS(6);
    dynamic PT_DYNAMIC FLAGS(6);
    stack PT_GNU_STACK FLAGS(6);
}

SECTIONS
{
    . = 0x100;
    .text : { *(.text) } :text
    . = 0x1000;
    .text.secondary : { *(.text.secondary) } :text

    . = 0x2000;
    .rodata : { *(.rodata) } :rodata

    . = 0x3000;
    .dynamic : { *(.dynamic) } :data :dynamic
    .dynsym : { *(.dynsym) } :data
    .gnu.hash : { *(.gnu.hash) } :data
    .dynstr : { *(.dynstr) } :data
    .rela.dyn : { *(.rela.dyn) } :data
    .rela.plt : { *(.rela.plt) } :data
    .got : { *(.got) } :data
    .got.plt : { *(.got.plt) } :data
    .init_array : { *(.init_array) } :data
    .data : { *(.data) } :data

    /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame*) }
}
