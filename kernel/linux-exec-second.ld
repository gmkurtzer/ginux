ENTRY(ginux_linux_exec_target)

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
    . = 0x40f000 + SIZEOF_HEADERS;
    .headers : { BYTE(0) } :headers

    . = 0x410100;
    .text : { *(.text) *(.text.*) } :text
    ASSERT(SIZEOF(.text) > 0xf00, "Second Linux exec target must cross an RX page")
    ASSERT(SIZEOF(.text) <= 0x1f00, "Second Linux exec target exceeds its two-page RX profile")

    . = 0x412000;
    .data : { *(.data) } :data
    .bss : { *(.bss) } :data

    /DISCARD/ : { *(.comment) *(.note*) }
}
