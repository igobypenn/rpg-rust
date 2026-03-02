require 'ffi'

module NativeLib
  extend FFI::Library
  ffi_lib 'libmylib.so'

  attach_function :add_numbers, [:int, :int], :int
  attach_function :process_data, [:pointer, :size_t], :int
  
  callback :data_callback, [:pointer, :int], :void
  attach_function :register_callback, [:data_callback], :void
end

module NativeUtils
  extend FFI::Library
  ffi_lib 'libc.so.6'
  
  attach_function :malloc, [:size_t], :pointer
  attach_function :free, [:pointer], :void
end

def add_via_ffi(a, b)
  NativeLib.add_numbers(a, b)
end
