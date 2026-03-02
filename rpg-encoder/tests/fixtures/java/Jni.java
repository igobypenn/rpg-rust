package com.example.jni;

import java.lang.foreign.*;
import java.lang.invoke.MethodHandle;

public class Jni {
    static {
        System.loadLibrary("mylib");
    }

    public native int addNumbers(int a, int b);
    public native String processData(String input);
    private native void internalMethod();

    public void callNative() {
        int result = addNumbers(10, 20);
        System.out.println("Result: " + result);
    }

    public static void main(String[] args) {
        Jni jni = new Jni();
        jni.callNative();
    }
}

class FFM {
    public void useFFM() throws Throwable {
        Linker linker = Linker.nativeLinker();
        SymbolLookup stdlib = linker.defaultLookup();
        
        MethodHandle strlen = linker.downcallHandle(
            stdlib.findOrThrow("strlen"),
            FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS)
        );
    }
}
