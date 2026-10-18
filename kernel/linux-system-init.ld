ENTRY(ginux_linux_system_init)

PHDRS
{
    phdr PT_PHDR PHDRS FLAGS(4);
    headers PT_LOAD FILEHDR PHDRS FLAGS(4);
    text PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
    stack PT_GNU_STACK FLAGS(6);
}

SECTIONS
{
    . = 0x414000 + SIZEOF_HEADERS;
    .headers : { BYTE(0) } :headers

    . = 0x415100;
    .text : { *(.text) *(.text.*) } :text
    ASSERT(SIZEOF(.text) <= 0x3f00, "Minimal system init exceeds its four-page RX profile")

    . = 0x419000;
    .data : { *(.data) } :data
    .bss : { *(.bss) } :data

    /DISCARD/ : { *(.comment) *(.note*) }
}
