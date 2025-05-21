console.log("user Js", "Hello World, from JS");

Runtime.serve(async (req) => {
  console.log("oiii");
  console.log("handle request: ", req);
  console.log("handle method: ", await req.method());

  const text = await req.text();

  console.log("handle request: ", text);

  return "ok";
});
