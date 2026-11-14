import init, {hello} from "ecc-wasm";

async function main() {
  await init();
  console.log(hello(42n));
}

main();
