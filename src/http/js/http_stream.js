import {
  op_http_get_method,
  op_http_read_request_body_text,
  op_http_set_promise_complete,
} from "ext:core/ops";

/** A wrapper of HttpStream to provide Request interface */
export class Request {
  /** @type HttpStream */
  #inner;
  #method;

  constructor(stream) {
    const isStream = stream instanceof HttpStream;
    if (!isStream) {
      throw new TypeError(
        `Cannot create Request: stram must be a 'HttpStream', received ${typeof stream}`,
      );
    }

    console.log("New Request");
    this.#inner = stream;
  }

  async text() {
    await this.#inner.text();
  }

  async method() {
    if (!this.#method) {
      this.#method = await this.#inner.method();
    }

    return this.#method;
  }
}

export class HttpStream {
  #rid;

  constructor(rid) {
    console.log("New HttpStream");
    this.#rid = rid;
  }

  async text() {
    await op_http_read_request_body_text(this.#rid);
  }

  async method() {
    return await op_http_get_method(this.#rid);
  }

  async complete() {
    await op_http_set_promise_complete(this.#rid, 200);
  }
}
