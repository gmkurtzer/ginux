#!/bin/sh
set -eu

document=docs/how-ginux-differs.md
test -s "$document"
test -s docs/security-boundary.md
test -s hdae/ARCHITECTURE.md

# Keep the architecture explanation discoverable from its three entry points,
# but do not make editorial wording part of the build interface.
grep -Fq '(docs/how-ginux-differs.md)' README.md
grep -Fq '(how-ginux-differs.md)' docs/security-boundary.md
grep -Fq '(../docs/how-ginux-differs.md)' hdae/ARCHITECTURE.md

printf '%s\n' 'check-architecture-doc: PASS (architecture explanation exists and remains linked)'
