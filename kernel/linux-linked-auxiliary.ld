ENTRY(ginux_auxiliary_symbol)

PHDRS
{
    text PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
    dynamic PT_DYNAMIC FLAGS(6);
    tls PT_TLS FLAGS(4);
    stack PT_GNU_STACK FLAGS(6);
}

SECTIONS
{
    . = 0x100;
    .text : { *(.text) *(.text.*) } :text

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
    .tdata : { *(.tdata) } :data :tls
    .tbss (NOLOAD) : { *(.tbss) } :data :tls

    /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame*) }
}
