ENTRY(ginux_linux_conformance)

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
    . = 0x3ff000 + SIZEOF_HEADERS;
    .headers : { BYTE(0) } :headers

    . = 0x400100;
    .text :
    {
        *(.text)
        *(.text.*)
    } :text
    ASSERT(SIZEOF(.text) <= 0x4f00, "Linux conformance probe exceeds its five-page RX profile")

    . = ALIGN(0x1000);
    .data : { *(.data) } :data
    .bss : { *(.bss) } :data

    /DISCARD/ : { *(.comment) *(.note*) }
}
