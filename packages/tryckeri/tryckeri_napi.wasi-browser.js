import {
  createOnMessage as __wasmCreateOnMessageForFsProxy,
  getDefaultContext as __emnapiGetDefaultContext,
  instantiateNapiModuleSync as __emnapiInstantiateNapiModuleSync,
  WASI as __WASI,
} from '@napi-rs/wasm-runtime'



const __wasi = new __WASI({
  version: 'preview1',
})

const __wasmUrl = new URL('./tryckeri_napi.wasm32-wasi.wasm', import.meta.url).href
const __emnapiContext = __emnapiGetDefaultContext()


const __sharedMemory = new WebAssembly.Memory({
  initial: 4000,
  maximum: 65536,
  shared: true,
})

const __wasmFile = await fetch(__wasmUrl).then((res) => res.arrayBuffer())

const {
  instance: __napiInstance,
  module: __wasiModule,
  napiModule: __napiModule,
} = __emnapiInstantiateNapiModuleSync(__wasmFile, {
  context: __emnapiContext,
  asyncWorkPoolSize: 4,
  wasi: __wasi,
  onCreateWorker() {
    const worker = new Worker(new URL('./wasi-worker-browser.mjs', import.meta.url), {
      type: 'module',
    })

    return worker
  },
  overwriteImports(importObject) {
    importObject.env = {
      ...importObject.env,
      ...importObject.napi,
      ...importObject.emnapi,
      memory: __sharedMemory,
    }
    return importObject
  },
  beforeInit({ instance }) {
    for (const name of Object.keys(instance.exports)) {
      if (name.startsWith('__napi_register__')) {
        instance.exports[name]()
      }
    }
  },
})
export default __napiModule.exports
export const applyMutations = __napiModule.exports.applyMutations
export const compileMdx = __napiModule.exports.compileMdx
export const compileMdxFromBuffer = __napiModule.exports.compileMdxFromBuffer
export const compileMdxFromBufferHighlighted = __napiModule.exports.compileMdxFromBufferHighlighted
export const compileMdxHighlighted = __napiModule.exports.compileMdxHighlighted
export const getBufferFormat = __napiModule.exports.getBufferFormat
export const hastBufferToHtmlStr = __napiModule.exports.hastBufferToHtmlStr
export const mdastBufferToHastBuffer = __napiModule.exports.mdastBufferToHastBuffer
export const parseMdxToBuffer = __napiModule.exports.parseMdxToBuffer
export const parseMdxToHastBuffer = __napiModule.exports.parseMdxToHastBuffer
export const parseMdxToHtml = __napiModule.exports.parseMdxToHtml
export const parseToBuffer = __napiModule.exports.parseToBuffer
export const parseToHastBuffer = __napiModule.exports.parseToHastBuffer
export const parseToHtml = __napiModule.exports.parseToHtml
