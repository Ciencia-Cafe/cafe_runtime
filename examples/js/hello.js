console.log("Hello World, from JS");

console.log(Object.getOwnPropertyNames(globalThis.Deno.core.ops));
console.log(Object.getOwnPropertyNames(Runtime));

Runtime.serve();
