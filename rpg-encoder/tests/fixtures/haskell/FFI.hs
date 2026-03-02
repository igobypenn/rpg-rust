{-# LANGUAGE ForeignFunctionInterface #-}

module MyApp.FFI where

import Foreign.C.Types
import Foreign.Ptr
import Foreign.Marshal.Alloc

foreign import ccall "add_numbers"
    c_add_numbers :: CInt -> CInt -> CInt

foreign import ccall "process_data"
    c_process_data :: Ptr CChar -> CSize -> CInt

foreign import ccall unsafe "malloc"
    c_malloc :: CSize -> IO (Ptr ())

foreign import ccall unsafe "free"
    c_free :: Ptr () -> IO ()

foreign export ccall
    haskell_export :: CInt -> CInt

haskell_export :: CInt -> CInt
haskell_export x = x * 2

addViaFfi :: Int -> Int -> Int
addViaFfi a b = fromIntegral (c_add_numbers (fromIntegral a) (fromIntegral b))

{-# LANGUAGE CPP #-}
