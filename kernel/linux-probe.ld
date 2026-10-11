ENTRY(ginux_linux_probe)

PHDRS
{
    phdr PT_PHDR PHDRS FLAGS(4);
    headers PT_LOAD FILEHDR PHDRS FLAGS(4);
    text PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
    relro PT_GNU_RELRO FLAGS(4);
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
        FILL(0)
        . = MAX(., 0x401000);
        BYTE(0)
    } :text
    ASSERT(SIZEOF(.text) <= 0x1f00, "Linux probe exceeds two-page executable profile")

    . = 0x402000;
    .data : { *(.data) } :data
    .bss : { *(.bss) } :data

    . = 0x403000;
    .data.rel.ro : { *(.data.rel.ro) } :data :relro

    /DISCARD/ : { *(.comment) *(.note*) }
}
