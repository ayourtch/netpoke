/* tslint:disable */
/* eslint-disable */

/**
 * Analyze the network path (traceroute) once and then close connections
 */
export function analyze_path(): Promise<void>;

export function main(): void;

export function start_measurement(): Promise<void>;

/**
 * Stop the measurement and release the wake lock
 */
export function stop_measurement(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly analyze_path: () => any;
  readonly main: () => void;
  readonly start_measurement: () => any;
  readonly stop_measurement: () => void;
  readonly wasm_bindgen__convert__closures_____invoke__h287429702d7d323a: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__h0a5100af4066f9a8: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__hb398a6691e2c7e8f: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__h517235567eb3b208: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h095e15d69fdcd2b9: (a: number, b: number) => void;
  readonly wasm_bindgen__closure__destroy__hbca06e3c0f65adfd: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h128ae0e8730c21bf: (a: number, b: number, c: any, d: any) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_externrefs: WebAssembly.Table;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
