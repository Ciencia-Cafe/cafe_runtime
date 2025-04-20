import { core, primordials } from "ext:core/mod.js";
import * as http from "ext:runtime_http/01.js";

const ops = core.ops;

const {
  ObjectKeys,
  ObjectDefineProperty,
} = primordials;

globalThis.bootstrap = {
  mainRuntime: mainRuntimeBootstrap,
};

const runtimeNs = {
  serve: http.Http.serve,
};

function mainRuntimeBootstrap() {
  console.log("main", "bootstrap");

  // Remove bootstrapping and private data from the global scope
  removeImportedOps();
  delete globalThis.bootstrap;

  ObjectDefineProperty(globalThis, "Runtime", core.propReadOnly(runtimeNs));
}

function removeImportedOps() {
  const allOpNames = ObjectKeys(ops);
  for (let i = 0; i < allOpNames.length; i++) {
    const opName = allOpNames[i];
    delete ops[opName];
  }
}
