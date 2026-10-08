import assert from "node:assert/strict";
import { discoverConnectionStatus } from "../src/connectionDiscovery.ts";
const source = { id:"data-out", direction:"out", channel:"data", contract:"c:abi:int32" };
const a = {id:"a",label:"A",ports:[{id:"in",direction:"in",channel:"data",contract:"c:abi:int32"}]};
const b = {id:"b",label:"B",ports:[{id:"in",direction:"in",channel:"data",contract:"python:ctypes:c_int"}]};
const c = {id:"c",label:"C",ports:[{id:"in",direction:"in",channel:"flow"}]};
const d = {id:"d",label:"D",ports:[{id:"in",direction:"in",channel:"data"}]};
const before = JSON.stringify([source,a,b,c,d]);
assert.equal(discoverConnectionStatus(source,a),"candidate-unverified");
assert.equal(discoverConnectionStatus(source,b),"needs-mapper-or-route");
assert.equal(discoverConnectionStatus(source,c),"no-direct-input");
assert.equal(discoverConnectionStatus(source,d),"insufficient-information");
assert.equal(discoverConnectionStatus(undefined,a),"select-output");
assert.equal(JSON.stringify([source,a,b,c,d]),before);
assert.ok(!["verified","compatible"].includes(discoverConnectionStatus(source,a)),
  "exact string contract is never semantic route proof");
console.log("PASS contextual discovery never claims unverified route as compatible");
