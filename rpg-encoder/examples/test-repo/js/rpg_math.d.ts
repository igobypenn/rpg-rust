/**
 * RPG Math library TypeScript declarations.
 */

export interface Config {
    precision: number;
    roundingMode: number;
}

export interface MathClientOptions {
    defaultPrecision?: number;
}

export function add(a: number, b: number): number;
export function multiply(a: number, b: number): number;
export function subtract(a: number, b: number): number;
export function process(value: number, config?: Config): number;
export function greet(name?: string): string;
export function validateInput(value: number): boolean;
export function safeProcess(value: number, precision?: number): number | null;
export function batchProcess(values: number[], precision?: number): number[];

export class MathClient {
    constructor(options?: MathClientOptions);
    add(a: number, b: number): number;
    multiply(a: number, b: number): number;
    process(value: number): number;
    setDefaultPrecision(p: number): void;
}
