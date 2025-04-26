import { op_http_set_promise_complete } from "ext:core/ops";

export class Request {
  #rid;

  constructor(rid) {
    console.log("New Request");
    this.#rid = rid;
  }

  async text() {
  }

  async complete() {
    await op_http_set_promise_complete(this.#rid, 200);
  }
}
