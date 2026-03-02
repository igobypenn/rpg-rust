/**
 * Java JNI bindings for RPG Math library.
 * 
 * This class provides Java wrappers around the native RPG Math functions
 * implemented in Rust. The native library must be loaded before using
 * any of the native methods.
 * 
 * @author RPG Team
 * @version 1.0.0
 */
package com.rpg.math;

public class MathClient {
    
    /**
     * Load the native library.
     * 
     * The library name is "rpg_math" and should be available in the
     * system library path or specified via -Djava.library.path
     */
    static {
        System.loadLibrary("rpg_math");
    }
    
    /**
     * Adds two integers via native code.
     * 
     * @param a The first integer
     * @param b The second integer
     * @return The sum of a and b
     */
    public native int add(int a, int b);
    
    /**
     * Multiplies two integers via native code.
     * 
     * @param a The first integer
     * @param b The second integer
     * @return The product of a and b
     */
    public native int multiply(int a, int b);
    
    /**
     * Subtracts b from a via native code.
     * 
     * @param a The first integer
     * @param b The second integer
     * @return a minus b
     */
    public native int subtract(int a, int b);
    
    /**
     * Processes a value with the given configuration.
     * 
     * @param value The input value to process
     * @param config The processing configuration
     * @return The processed result
     */
    public native int process(int value, Config config);
    
    /**
     * Creates a greeting string.
     * 
     * @param name The name to greet
     * @return A greeting string
     */
    public native String greet(String name);
    
    /**
     * Initializes the library.
     * 
     * @return true if initialization succeeded
     */
    public native boolean init();
    
    /**
     * Shuts down the library.
     */
    public native void shutdown();
    
    /**
     * Configuration for complex operations.
     */
    public static class Config {
        /** Precision multiplier for processing */
        public int precision;
        
        /** Rounding mode: 0 for integer, 1 for float */
        public int roundingMode;
        
        /**
         * Creates a new Config with default values.
         */
        public Config() {
            this.precision = 1;
            this.roundingMode = 0;
        }
        
        /**
         * Creates a new Config with specified values.
         * 
         * @param precision The precision multiplier
         * @param roundingMode The rounding mode
         */
        public Config(int precision, int roundingMode) {
            this.precision = precision;
            this.roundingMode = roundingMode;
        }
    }
    
    /**
     * High-level client with simplified API.
     */
    public static class Builder {
        private int defaultPrecision = 1;
        
        /**
         * Sets the default precision for operations.
         * 
         * @param precision The default precision
         * @return This builder instance
         */
        public Builder withDefaultPrecision(int precision) {
            this.defaultPrecision = precision;
            return this;
        }
        
        /**
         * Builds and initializes a new MathClient.
         * 
         * @return A new MathClient instance
         */
        public MathClient build() {
            MathClient client = new MathClient();
            client.init();
            return client;
        }
    }
}
