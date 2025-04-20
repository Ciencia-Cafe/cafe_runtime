const { core } = Deno;

export class Http {
  #rid;

  constructor(rid) {
    this.#rid = rid;
  }

  static async serve() {
    const rid = await core.ops.op_http_serve();
    // const http = new Http(rid);

    console.log("Serving http server", rid);

    while (true) {
      await core.ops.op_http_accept(rid);
    }
  }
}

globalThis.runtime = {
  ...globalThis.runtime,
  Http,
};
