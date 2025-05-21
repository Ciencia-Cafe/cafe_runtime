import { op_http_serve, op_http_wait } from "ext:core/ops";
import { HttpStream, Request } from "ext:runtime_http/http_stream.js";

export class Http {
  #rid;

  constructor(rid) {
    this.#rid = rid;
  }

  static async serve(handler) {
    if (typeof handler !== "function" || !handler) {
      throw new TypeError(
        `Cannot serve HTTP requests: handler must be a function, received ${typeof handler}`,
      );
    }

    const rid = await op_http_serve();
    // const http = new Http(rid);

    console.log("Serving http server", rid);

    while (true) {
      const reqRid = await op_http_wait(rid);
      if (reqRid) {
        const stream = new HttpStream(reqRid);
        console.log("receive request:", stream);

        const request = new Request(stream);
        const response = await handler(request);

        console.log("got response:", response);

        await stream.complete();
      }
    }
  }
}
