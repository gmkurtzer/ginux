PHDRS
{
  load PT_LOAD FILEHDR PHDRS FLAGS(5);
  dynamic PT_DYNAMIC FLAGS(4);
}

SECTIONS
{
  . = SIZEOF_HEADERS;
  .hash          : { *(.hash) }          :load
  .gnu.hash      : { *(.gnu.hash) }      :load
  .dynsym        : { *(.dynsym) }        :load
  .dynstr        : { *(.dynstr) }        :load
  .gnu.version   : { *(.gnu.version) }   :load
  .gnu.version_d : { *(.gnu.version_d) } :load
  .dynamic       : { *(.dynamic) }       :load :dynamic
  .text          : { *(.text*) }         :load
  /DISCARD/      : { *(.comment) *(.eh_frame*) *(.note.GNU-stack) }
}
