import { op_http_accept, op_http_serve } from "ext:core/ops";

export class Http {
  #rid;

  constructor(rid) {
    this.#rid = rid;
  }

  static async serve() {
    const rid = await op_http_serve();
    // const http = new Http(rid);

    console.log("Serving http server", rid);

    while (true) {
      await op_http_accept(rid);
    }
  }
}
