/**
 * @fileoverview RPG Math library bindings for Node.js
 * 
 * This module provides Node.js bindings for the RPG Math library
 * using N-API (Node-API).
 */

const path = require('path');

let binding = null;

/**
 * Load the native binding.
 */
function loadBinding() {
    if (binding) {
        return binding;
    }

    const bindingPath = path.join(
        __dirname,
        '..',
        'rust',
        'target',
        'release',
        `rpg_math_node.${process.platform === 'win32' ? 'dll' : 'so'}`
    );

    try {
        binding = require(bindingPath);
    } catch (e) {
        // Try system library
        try {
            binding = require('rpg_math_node');
        } catch (e2) {
            throw new Error('Could not load RPG Math native binding');
        }
    }

    return binding;
}

/**
 * Adds two integers.
 * 
 * @param {number} a - First integer
 * @param {number} b - Second integer
 * @returns {number} The sum of a and b
 */
function add(a, b) {
    const lib = loadBinding();
    return lib.rpg_add(a, b);
}

/**
 * Multiplies two integers.
 * 
 * @param {number} a - First integer
 * @param {number} b - Second integer
 * @returns {number} The product of a and b
 */
function multiply(a, b) {
    const lib = loadBinding();
    return lib.rpg_multiply(a, b);
}

/**
 * Subtracts b from a.
 * 
 * @param {number} a - First integer
 * @param {number} b - Second integer
 * @returns {number} a minus b
 */
function subtract(a, b) {
    const lib = loadBinding();
    return lib.rpg_subtract(a, b);
}

/**
 * @typedef {Object} Config
 * @property {number} precision - Precision multiplier
 * @property {number} roundingMode - 0 for integer, 1 for float
 */

/**
 * Processes a value with configuration.
 * 
 * @param {number} value - The input value
 * @param {Config} config - Configuration object
 * @returns {number} The processed result
 */
function process(value, config = { precision: 1, roundingMode: 0 }) {
    const lib = loadBinding();
    return lib.rpg_process(value, config.precision, config.roundingMode);
}

/**
 * Creates a greeting string.
 * 
 * @param {string} name - The name to greet
 * @returns {string} A greeting string
 */
function greet(name = 'World') {
    const lib = loadBinding();
    return lib.rpg_greet(name);
}

/**
 * Validates input values.
 * 
 * @param {number} value - The value to validate
 * @returns {boolean} True if valid
 */
function validateInput(value) {
    return Number.isInteger(value) && value >= 0 && value <= 10000;
}

/**
 * Safely processes a value with validation.
 * 
 * @param {number} value - The input value
 * @param {number} precision - Precision multiplier
 * @returns {number|null} The result or null if invalid
 */
function safeProcess(value, precision = 1) {
    if (!validateInput(value)) {
        return null;
    }
    return process(value, { precision, roundingMode: 0 });
}

/**
 * Batch processes multiple values.
 * 
 * @param {number[]} values - Array of values to process
 * @param {number} precision - Precision multiplier
 * @returns {number[]} Array of results
 */
function batchProcess(values, precision = 1) {
    return values.map(v => safeProcess(v, precision));
}

/**
 * High-level client for RPG Math operations.
 */
class MathClient {
    /**
     * Creates a new MathClient.
     * @param {Object} options - Client options
     * @param {number} options.defaultPrecision - Default precision for operations
     */
    constructor(options = {}) {
        this.defaultPrecision = options.defaultPrecision || 1;
        this._lib = loadBinding();
    }

    /**
     * Adds two numbers.
     * @param {number} a - First number
     * @param {number} b - Second number
     * @returns {number} Sum of a and b
     */
    add(a, b) {
        return add(a, b);
    }

    /**
     * Multiplies two numbers.
     * @param {number} a - First number
     * @param {number} b - Second number
     * @returns {number} Product of a and b
     */
    multiply(a, b) {
        return multiply(a, b);
    }

    /**
     * Processes a value with default precision.
     * @param {number} value - Input value
     * @returns {number} Processed result
     */
    process(value) {
        return process(value, { precision: this.defaultPrecision, roundingMode: 0 });
    }

    /**
     * Sets the default precision.
     * @param {number} p - New default precision
     */
    setDefaultPrecision(p) {
        this.defaultPrecision = p;
    }
}

module.exports = {
    add,
    multiply,
    subtract,
    process,
    greet,
    validateInput,
    safeProcess,
    batchProcess,
    MathClient,
};
