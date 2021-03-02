# Better-Alignment
Better alignment algorithms.

You will probably need these programs on your `$PATH`:
* wasmtime
* wasmer
* binaryen wasm-opt
* wabt wasm2wat

## WASM SIMD support
[ ] Use bitmask instruction instead of workaround
[ ] Make sure functions without target feature attribute are inlined correctly.
Easy fix: run binaryen wasm-opt pass with lots of inlining.
