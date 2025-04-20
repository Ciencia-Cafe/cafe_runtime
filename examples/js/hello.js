console.log("Hello World, from JS");

console.log(Object.getOwnPropertyNames(Deno.core.ops));

runtime.Http.serve();
