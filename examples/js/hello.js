console.log("user Js", "Hello World, from JS");

const log = (...args) => console.log("[JS|user]", ...args);

Runtime.serve(async (req) => {
  log("req: ", req);
  log("req.method: ", await req.method());

  const text = await req.text();

  log("req.text: ", text);

  return "ok";
});
