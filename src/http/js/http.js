import { op_http_serve, op_http_wait } from "ext:core/ops";
import { Request } from "ext:runtime_http/request.js";

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
      const reqRid = await op_http_wait(rid);
      if (reqRid) {
        const request = new Request(reqRid);
        console.log("receive request:", request);

        await request.complete();
      }
    }
  }
}
