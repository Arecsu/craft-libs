# Shared Hayro reader snapshot

The `hayro`, `hayro-interpret`, `hayro-syntax` and `hayro-jbig2` directories are transferred
from [PdfCraft revision 7437bde378f43856eada40af25b983812e615e8a](https://github.com/storytold/pdfcraft/tree/7437bde378f43856eada40af25b983812e615e8a/vendor).
The upstream [patch ledger](https://github.com/storytold/pdfcraft/blob/7437bde378f43856eada40af25b983812e615e8a/vendor/README.md)
describes the inherited patches. Source manifests record every transferred Git blob and
SHA-256 alongside the local hash. The source manifests themselves are not self-hashed.

`hayro-cmap` 0.1.0 is transferred from [PdfCraft revision a2ec27b4775f53c40cad3456936d9406611e3a8d](https://github.com/storytold/pdfcraft/tree/a2ec27b4775f53c40cad3456936d9406611e3a8d/vendor/hayro-cmap)
with its malformed-CMap patch (PdfCraft #3177): bad `bfchar` or `bfrange` entries, an
unresolvable `usecmap` and an unparseable tail are skipped instead of discarding the whole
CMap. Its tests `tests::bad_bfchar_entry_is_skipped` and `tests::unresolvable_usecmap_is_skipped`
come with it. One local patch: a binary `usecmap` counts against the 16-level nesting limit
as a text one does, so a self-referencing binary CMap stops instead of overflowing the stack.
Consuming workspaces need a `hayro-cmap` patch entry beside the other Hayro entries.

`hayro-postscript` 0.1.0 is the crates.io package (upstream revision
a2354aebaf86f5ab13e5fd6b6b0521306488bf9d), the PostScript token scanner `hayro-cmap` uses
to parse the CMap streams PDFs embed. One local patch: the scanner, which does not parse
dictionaries yet, skips runs of `<<` and `>>` in a loop and returns the next object, instead
of recursing once per delimiter, so a long run of `<` or `>` bytes no longer overflows the
stack. Tests: `object::tests::long_open_dict_run_does_not_overflow`,
`object::tests::long_close_dict_run_does_not_overflow`, `object::tests::dict_delimiters_are_skipped`
and `object::tests::nested_dict_delimiters_are_skipped`. Its `SOURCE.json` records the
registry hashes. Consuming workspaces need a `hayro-postscript` patch entry beside the other
Hayro entries.

The shared snapshot adds immutable per-document ordinary decode limits, threaded through
initial xrefs, object streams, fallback and repair. Existing constructors retain their
previous default. Known-size image budgets and streaming content budgets remain distinct.
ContentReader now reports omitted or failed whole streams and incomplete Flate blocks as
known incomplete content. The interpreter also reports failed or budget-reaching nested
forms, patterns and Type 3 procedures. A stream exposes its immutable document limit so
callers can use the same conservative completion check. This diagnostic does not cover
structural repair or every omitted object and is not a rendering-completeness guarantee.
The syntax manifest pins JBIG2 exactly to 0.3.0 so a newer registry release cannot bypass
the transferred decoder patches.
Synthetic unit tests cover these boundaries; two version tests and the introductory doctest
use synthetic PDFs so they do not rely on the absent upstream corpus. No corpus is copied.

Compatibility patches follow the resource scopes and text-showing order in
[Hayro 0.8.0, revision ea9c81dc70cde40c64205986fdcfcd481fe934d8](https://github.com/LaurenzV/hayro/tree/ea9c81dc70cde40c64205986fdcfcd481fe934d8),
under the same MIT OR Apache-2.0 licence. Explicit page and nested-object resource
dictionaries replace the enclosing dictionary as a whole. Missing Form resources inherit
the caller; tiling patterns use only their own resources. Direct arrays and dictionaries
are limited to 64 levels in raw and decoded content. Page-tree traversal is iterative,
retains cycle detection and inheritance, and fails closed when its 100,000-entry work
budget or a fallible frame/page allocation fails. Text-showing operations fill the
whole run before stroking it, including strings separated by adjustments in `TJ`.
The implementation replays bounded source operands without allocating a glyph buffer.
The existing Type 3 single-paint behavior and incomplete-content diagnostics remain.
Raw array iteration now ends safely if validation and object parsing disagree;
standalone sign tokens retain the reader's zero tolerance.
Byte transfer-function samples use rounded 8-bit values; failed or non-finite evaluations
leave the sample unchanged. Historical source hashes remain in each `SOURCE.json`,
alongside local hashes and the compatibility reference.

The JBIG2 segment parser caps the long-form referred-to-segment count at `u16::MAX`
and rejects a larger claim as an invalid count before any retention-flag or
referred-to-number bytes are consumed, so a crafted header cannot turn
`Vec::with_capacity` into a multi-gigabyte reservation that aborts on allocation
failure.

Hostile-input bounds (CodeRabbit findings):

- An xref-stream subsection must be backed by its decoded bytes: all-zero `/W` widths
  are refused, and a declared count longer than the data is refused before any entry is
  inserted, so a tiny stream cannot declare billions of entries.
- Each object stream's `/N` header pairs are parsed once per document and cached beside its
  decoded data, instead of on every compressed-object lookup.
- An embedded JBIG2 page over 2^28 pixels is refused before the output buffer is allocated,
  and that buffer is reserved fallibly (`Bitmap::reinitialize` allocates before touching the
  existing bitmap, so a failed reservation leaves it intact). A `JBIG2Globals` stream with an image filter is
  ignored, so globals cannot recurse through further globals.
- At most 4096 graphics states, holding at most 64 MiB together, are saved; a `q` past
  either limit is counted but not copied, and its `Q` still removes the clips added since.
- An xref-stream `/W` type field wider than one byte is read at its full width (big-endian)
  instead of only its first byte, and `/W[0]` over 8 bytes is refused like `/W[1]`.
  A dash array over 256 entries strokes solid.
- An xref-stream subsection running past object number `i32::MAX` is refused instead of
  overflowing or turning negative.
- A CID font's `/W2` run that starts past CID 65535 is skipped as in `/W`, instead of
  overflowing and discarding the whole table.
- CCITT images: `ccitt_size_ok` (inherited PdfCraft ledger item 2) allows at most 2^20
  columns and 2^28 pixels (columns × rows) before the decoder allocates. A `/Columns` past
  `u32::MAX` is refused rather than wrapped to a small width. `/Rows` follows PdfCraft (17)
  and (19): `/Rows 0` or a value below `/Height` decodes to the image height, and decoding
  still stops where the data ends.

Reader limits from the [PdfCraft revision a2ec27b4775f53c40cad3456936d9406611e3a8d ledger](https://github.com/storytold/pdfcraft/blob/a2ec27b4775f53c40cad3456936d9406611e3a8d/vendor/README.md),
by its item ids: hayro-interpret (31), its overflow part: the largest 32-bit Type 0 sample is
computed in 64 bits instead of overflowing `2_u32.pow(32)`. The flat sample array that (31)
also brings is a performance change and is not part of this set. hayro-syntax (7), (22) and
(25) are covered by the local bounds above: ASCII85 output stops at the decode limit,
object-stream indexes are parsed once with checked offsets, and direct objects nest at most
64 levels, stricter than PdfCraft's 128. Test:
`function::type0::tests::thirty_two_bit_samples_do_not_overflow`.

Rendering fixes from the same [PdfCraft revision a2ec27b4775f53c40cad3456936d9406611e3a8d ledger](https://github.com/storytold/pdfcraft/blob/a2ec27b4775f53c40cad3456936d9406611e3a8d/vendor/README.md),
by its item ids, each marked `PdfCraft patch` in the source: hayro-syntax (17) and (19), CCITT
`/Rows 0` or fewer `/Rows` than `/Height` decode to the image height; (20), spaces or tabs
between `stream` and its line break; the `EI` extension of (4), `EI` followed by a delimiter
ends an inline image; (23), bytes above 0x7F after an `EI` candidate don't reject it; the
unledgered #3565 fix, a doubled sign (`--280`) is one number in both read and skip, beside the
local zero-tolerance sign tokens; and the `base_dimensions` one-point floor after `/UserUnit`.
Their unit tests come with them; the CCITT raster tests are adapted in `tests/upstream_raster.rs`.
Text fixes from the same ledger in hayro-interpret: (30), the Annex D.2 alias codes (WinAnsi
0xA0 and 0xAD, MacRoman 0xCA) read as an ordinary space and hyphen unless `/Differences` names
them; (44), a `/ToUnicode /Identity-H` or `/Identity-V` name reads two-byte codes as Unicode;
(P6), glyph names outside the Adobe Glyph List (`f_i`, `hyphen.case`, `uniXXXXXXXX`) read by
the glyph list specification's full mapping; (P8), a substitute without a spacing character
draws its ordinary space; (P2), a Type 3 glyph's `/FontBBox` is taken to device space before it
bounds the glyph's clip and shadings; and the extension of (26), comma-below letters (ș ț ķ ļ ņ
ŗ Ģ) in the standard-font stand-ins are built from the font's comma. The text contracts are
adapted in `tests/upstream_text.rs` and the Type 3 and comma-below raster tests in
`tests/upstream_raster.rs`. (P8)'s PdfCraft test needs a Japanese substitute font and is not
ported.

This remains the patched 0.7 backend. It retains the existing Bradford Lab conversion,
floating component precision and codec versions, rather than claiming full 0.8
equivalence. The independent color oracles, backend tests and facade contracts cover
these retained policies separately from the compatibility changes.

All other source and asset files remain unchanged. Upstream licence texts, Foxit/PDFium
standard-14 fonts and the CMYK ICC profile are retained. Attribution is in the root NOTICE
and ATTRIBUTION.md. The removed LAB.icc profile remains absent. Lopdf is not part of this
snapshot: applications that need its editing or inspection compatibility keep owning it.

Root Cargo patch entries are required in consuming workspaces. The writer-only feature
remains independent; vendoring reader sources does not make them a writer dependency.

The transferred snapshot includes the latest Lab Bradford adaptation, Type 3 rendering-mode
handling and filtered shrinking of oversized soft masks. `tests/upstream_raster.rs` adapts
the corresponding portable PdfCraft rendering contracts for native and WASM.
The snapshot also preserves PdfCraft #1477 (WinAnsi Unicode fallback for TrueType
fonts without Encoding) and #1499 (byte-aligned packed TIFF predictor rows).
Synthetic tests cover 1/2/4/8-bit RGB rows and rendering, plus embedded TrueType
text with absent or partial ToUnicode maps and explicit encoding precedence.
