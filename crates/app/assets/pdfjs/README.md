# pdf.js (bundled)

Mozilla's pdf.js, **version 6.4.299** (Apache-2.0, see `LICENSE`; the decoders in `wasm/`
have their own licences there). The app's PDF viewer draws pages with its core library
(phase-2.md 8f, decision #037; the human approved this version 2026-11-04).

- From the npm package `pdfjs-dist@6.4.299`
  (`https://registry.npmjs.org/pdfjs-dist/-/pdfjs-dist-6.4.299.tgz`), checked against the
  registry's published integrity:
  `sha512-AVl138zALtfaAPvADulE0PZThbYzCBS79nL4pOSL/6Sm/4AH5A21BD9VHt97OlCuzJuCpmeZtAtkinisF4Vb1g==`
- **Copied, unchanged:** `build/pdf.min.mjs`, `build/pdf.worker.min.mjs` (here at the top),
  `cmaps/`, `standard_fonts/`, `iccs/`, and from `wasm/` the JBIG2, OpenJPEG, and QCMS decoders
  with their licences. **Left out:** the form-scripting engine (QuickJS), the sandbox, the
  legacy build, the viewer app, and the source maps.
- **Updating:** download the new version's tarball, check its integrity against the registry,
  unpack it in a scratch folder, copy the same files over these, and change this file.
