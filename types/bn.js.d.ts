declare module "bn.js" {
  export default class BN {
    constructor(number: number | string | BN, base?: number | "hex");
    toString(base?: number | "hex"): string;
    toNumber(): number;
    toJSON(): string;
    toArray(endian?: string, length?: number): number[];
    toBuffer(endian?: string, length?: number): Buffer;
    bitLength(): number;
    zeroBits(): number;
    byteLength(): number;
    isNeg(): boolean;
    isEven(): boolean;
    isOdd(): boolean;
    isZero(): boolean;
    cmp(b: BN): number;
    lt(b: BN): boolean;
    lte(b: BN): boolean;
    gt(b: BN): boolean;
    gte(b: BN): boolean;
    eq(b: BN): boolean;
    isBN(b: any): boolean;

    neg(): BN;
    abs(): BN;
    add(b: BN): BN;
    sub(b: BN): BN;
    mul(b: BN): BN;
    sqr(): BN;
    pow(b: BN): BN;
    div(b: BN): BN;
    mod(b: BN): BN;
    divRound(b: BN): BN;

    clone(): BN;
    shln(b: number): BN;
    shrn(b: number): BN;
    invm(b: BN): BN;
    gcd(b: BN): BN;
    egcd(b: BN): { a: BN; b: BN; gcd: BN };
    max(b: BN): BN;
    min(b: BN): BN;
  }
}
