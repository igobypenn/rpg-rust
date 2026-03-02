const ffi = require('ffi-napi');
const ref = require('ref-napi');

const libmylib = ffi.Library('libmylib', {
    'add_numbers': ['int', ['int', 'int']],
    'process_data': ['int', ['pointer', 'size_t']],
    'allocate_buffer': ['pointer', ['size_t']],
    'free_buffer': ['void', ['pointer']]
});

const myNativeAddon = require('./build/Release/mynative.node');

function addViaFfi(a, b) {
    return libmylib.add_numbers(a, b);
}

function callNative(a, b) {
    return myNativeAddon.add(a, b);
}

const wasmModule = new WebAssembly.Module(wasmBuffer);
const wasmInstance = new WebAssembly.Instance(wasmModule, imports);

WebAssembly.instantiateStreaming(fetch('module.wasm'), importObject)
    .then(result => {
        const add = result.instance.exports.add;
    });
