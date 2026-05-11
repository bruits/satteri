import {
  createOnMessage as __wasmCreateOnMessageForFsProxy,
  getDefaultContext as __emnapiGetDefaultContext,
  instantiateNapiModuleSync as __emnapiInstantiateNapiModuleSync,
  WASI as __WASI,
} from '@napi-rs/wasm-runtime'



const __wasi = new __WASI({
  version: 'preview1',
})

const __wasmUrl = new URL('./satteri_napi.wasm32-wasi.wasm', import.meta.url).href
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
export const applyCommandsAndConvertToHastHandle = __napiModule.exports.applyCommandsAndConvertToHastHandle
export const applyCommandsToHandle = __napiModule.exports.applyCommandsToHandle
export const applyCommandsToMdastHandle = __napiModule.exports.applyCommandsToMdastHandle
export const compileHandle = __napiModule.exports.compileHandle
export const compileMdx = __napiModule.exports.compileMdx
export const convertMdastToHastHandle = __napiModule.exports.convertMdastToHastHandle
export const createHastHandle = __napiModule.exports.createHastHandle
export const createMdastHandle = __napiModule.exports.createMdastHandle
export const createMdxHastHandle = __napiModule.exports.createMdxHastHandle
export const createMdxMdastHandle = __napiModule.exports.createMdxMdastHandle
export const dropHandle = __napiModule.exports.dropHandle
export const getHandleSource = __napiModule.exports.getHandleSource
export const getMdastFrontmatter = __napiModule.exports.getMdastFrontmatter
export const getNodeData = __napiModule.exports.getNodeData
export const mdastTextContentHandle = __napiModule.exports.mdastTextContentHandle
export const parseEsm = __napiModule.exports.parseEsm
export const parseExpression = __napiModule.exports.parseExpression
export const parseToHtml = __napiModule.exports.parseToHtml
export const renderHandle = __napiModule.exports.renderHandle
export const serializeHandle = __napiModule.exports.serializeHandle
export const setNodeData = __napiModule.exports.setNodeData
export const textContentHandle = __napiModule.exports.textContentHandle
export const walkHandle = __napiModule.exports.walkHandle
export const walkMdastHandle = __napiModule.exports.walkMdastHandle
