// Ambient types for the smoke specs and wdio.conf.ts: WebdriverIO's globals (`WebdriverIO.Browser`,
// `WebdriverIO.Config`, the mocha options) and Mocha's BDD functions. The six Mocha functions are
// declared here so the tree does not need `@types/mocha` just for the smoke specs.
/// <reference types="webdriverio" />
/// <reference types="@wdio/globals/types" />
/// <reference types="@wdio/mocha-framework" />

declare function describe(name: string, fn: () => void): void;
declare function it(name: string, fn: () => void | Promise<void>): void;
declare function before(fn: () => void | Promise<void>): void;
declare function after(fn: () => void | Promise<void>): void;
declare function beforeEach(fn: () => void | Promise<void>): void;
declare function afterEach(fn: () => void | Promise<void>): void;
